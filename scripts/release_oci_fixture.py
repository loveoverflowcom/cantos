"""Private network-namespace HTTP/SMTP driver for release oci-qualify.

Only the parent qualifier starts this helper. Credentials and captured mail stay in memory;
stdout contains assertion names only. No fake authentication/database adapter is used.
"""

import email
import email.policy
import http.client
import json
import queue
import re
import socket
import socketserver
import sys
import threading
import time
import urllib.parse


class CheckFailed(Exception):
    pass


def check(condition, name):
    if not condition:
        raise CheckFailed(name)


class MailSink(socketserver.ThreadingTCPServer):
    allow_reuse_address = True
    daemon_threads = True

    def __init__(self):
        self.messages = queue.Queue()
        super().__init__(("127.0.0.1", 2525), MailHandler)


class MailHandler(socketserver.StreamRequestHandler):
    def handle(self):
        self.connection.settimeout(10)
        self.wfile.write(b"220 fixture.example.test ESMTP\r\n")
        size = 0
        while True:
            line = self.rfile.readline(8193)
            size += len(line)
            if not line or size > 65536:
                return
            verb = line.split(b" ", 1)[0].strip().upper()
            if verb in (b"EHLO", b"HELO"):
                self.wfile.write(b"250 fixture.example.test\r\n")
            elif verb in (b"MAIL", b"RCPT", b"RSET", b"NOOP"):
                self.wfile.write(b"250 OK\r\n")
            elif verb == b"DATA":
                self.wfile.write(b"354 Send data\r\n")
                data = bytearray()
                while True:
                    part = self.rfile.readline(8193)
                    if part == b".\r\n":
                        break
                    if not part or len(data) + len(part) > 32768:
                        return
                    data.extend(part[1:] if part.startswith(b"..") else part)
                self.server.messages.put(bytes(data))
                self.wfile.write(b"250 Accepted\r\n")
            elif verb == b"QUIT":
                self.wfile.write(b"221 Bye\r\n")
                return
            else:
                self.wfile.write(b"502 Unsupported\r\n")


class Journey:
    base = "/api/v1/theatre/native/identity/"

    def __init__(self, sink):
        self.sink = sink
        self.pair = None
        self.revoked = False
        self.login = {"email": "qualification@example.test", "password": "Synthetic-only-Cantos-47!"}

    def http(self, method, path, expected, body=None, headers=None):
        connection = http.client.HTTPConnection("127.0.0.1", 8080, timeout=10)
        try:
            encoded = None if body is None else json.dumps(body)
            connection.request(method, path, encoded, {"Content-Type": "application/json", **(headers or {})})
            response = connection.getresponse()
            data = response.read(65537)
            check(len(data) <= 65536 and response.status == expected,
                  "http:" + method + ":" + path + ":expected-" + str(expected) + ":got-" + str(response.status))
            return (json.loads(data) if data else None), {k.lower(): v for k, v in response.getheaders()}
        finally:
            connection.close()

    def session(self, expected=200, headers=None):
        return self.http("GET", self.base + "session", expected,
                         headers={"Authorization": "Bearer " + self.pair["access_token"], **(headers or {})})[0]

    def health(self):
        # A real public production HTTP handler, not merely an open TCP port.
        self.http("GET", "/api/v1/theatre/works", 200)
        return ["production-http-public-handler"]

    def isolation(self, host_port):
        with socket.create_connection(("127.0.0.1", 2525), timeout=2) as connection:
            check(connection.recv(128).startswith(b"220"), "namespace-positive-control")
        try:
            socket.create_connection(("127.0.0.1", host_port), timeout=2).close()
        except OSError:
            pass
        else:
            raise CheckFailed("host-loopback-was-reachable")
        routes = open("/proc/net/route", encoding="ascii").read().splitlines()[1:]
        check(not any(line.split()[0] != "lo" for line in routes), "external-network-route")
        with open("/proc/net/ipv6_route", encoding="ascii") as source:
            check(all(line.split()[-1] == "lo" for line in source if line.strip()), "external-ipv6-route")
        return ["namespace-loopback-positive", "host-loopback-negative", "no-external-ipv4-or-ipv6-route"]

    def signup(self):
        self.http("GET", self.base + "session", 401)
        accepted, _ = self.http("POST", self.base + "signup", 202, {"email": self.login["email"]})
        check(accepted["accepted"] is True and accepted["proof_ttl_seconds"] > 0, "signup-accepted")
        message = email.message_from_bytes(self.sink.messages.get(timeout=15), policy=email.policy.default)
        body = message.get_content()
        match = re.search(r"https://theatre\.example\.test/signup#[^\s]+", body)
        check(match is not None, "real-smtp-proof-link")
        parts = urllib.parse.parse_qs(urllib.parse.urlsplit(match[0]).fragment, strict_parsing=True)
        command = {"operation_id": parts["operation"][0], "proof_id": parts["proof"][0],
                   "proof_secret": parts["secret"][0], "password": self.login["password"],
                   "explicit_confirmation": True}
        self.http("POST", self.base + "signup/complete", 400, {**command, "explicit_confirmation": False})
        self.http("POST", self.base + "signup/complete", 401, {**command, "proof_secret": "00" * 32})
        completed, _ = self.http("POST", self.base + "signup/complete", 200, command)
        check(completed == {"status": "completed"}, "signup-completed-without-session")
        self.http("GET", self.base + "session", 401)
        replay, _ = self.http("POST", self.base + "signup/complete", 200, command)
        check(replay == {"status": "replayed"}, "exact-completion-replay")
        self.http("POST", self.base + "login", 401, {**self.login, "password": "Incorrect-synthetic-47!"})
        self.http("POST", self.base + "login", 401, self.login, {"Origin": "https://theatre.example.test"})
        self.http("POST", "/api/v1/theatre/identity/login", 403, self.login,
                  {"Origin": "https://foreign.example.test"})
        self.pair, _ = self.http("POST", self.base + "login", 200, self.login)
        check(self.pair["token_type"] == "Bearer" and time.time() < self.pair["expires_at"] <=
              self.pair["absolute_expires_at"], "real-policy-token-lifetimes")
        check(self.session()["actor_id"] == self.pair["actor_id"], "authenticated-actor")
        self.session(401, {"Cookie": "unexpected=browser"})
        self.http("GET", "/api/v1/studio/identity/session", 401,
                  headers={"Authorization": "Bearer " + self.pair["access_token"]})
        web = "/api/v1/theatre/identity/session"
        origin = {"Origin": "https://theatre.example.test"}
        view, headers = self.http("POST", "/api/v1/theatre/identity/login", 200, self.login, origin)
        cookie = headers.get("set-cookie", "")
        check(view["actor_id"] == self.pair["actor_id"] and "access_token" not in view and
              all(flag in cookie for flag in ("HttpOnly", "Secure", "SameSite=Strict", "Path=/api/v1/theatre")),
              "web-private-cookie")
        browser = {**origin, "Cookie": cookie.split(";", 1)[0]}
        self.http("GET", web, 200, headers=browser)
        self.http("DELETE", web, 403, headers={"Cookie": browser["Cookie"]})
        self.http("DELETE", web, 204, headers=browser)
        self.http("GET", web, 401, headers=browser)
        old = self.pair
        self.pair, _ = self.http("POST", self.base + "refresh", 200,
                                {k: old[k] for k in ("refresh_token", "session_id")})
        check(self.pair["session_id"] == old["session_id"] and
              self.pair["refresh_token"] != old["refresh_token"], "refresh-rotation")
        self.session()
        return ["real-smtp-mailbox-proof", "confirmation-required", "wrong-proof-denied", "no-signup-session",
                "exact-replay", "wrong-password-denied", "native-browser-mix-denied", "foreign-origin-denied",
                "argon2-login", "native-session", "studio-surface-denied", "web-secure-cookie",
                "web-csrf-and-logout", "refresh-rotation", "policy-lifetimes"]

    def persisted(self):
        check(self.session()["actor_id"] == self.pair["actor_id"], "restored-session-actor")
        logged_in, _ = self.http("POST", self.base + "login", 200, self.login)
        check(logged_in["actor_id"] == self.pair["actor_id"], "restored-identity-password")
        return ["persisted-session-and-actor", "persisted-argon2-password"]

    def revoke(self):
        self.http("POST", self.base + "revoke", 204,
                  {k: self.pair[k] for k in ("refresh_token", "session_id")})
        self.session(401)
        return ["revoked-session-denied"]

    def unavailable(self):
        # Identity unavailable must never grant a session, even with a valid password.
        self.http("POST", self.base + "login", 503, self.login)
        return ["identity-unavailable-fails-closed"]

    def denied_peer(self):
        # Identity drops a disallowed OS peer before reading RPC bytes. That closed
        # transport maps to Unavailable/503, rather than an RPC password denial/401.
        self.http("POST", self.base + "login", 503, self.login)
        return ["identity-peer-transport-fails-closed"]


def main():
    sink = MailSink()
    threading.Thread(target=sink.serve_forever, daemon=True).start()
    journey = Journey(sink)
    print(json.dumps({"ready": True}), flush=True)
    for line in sys.stdin:
        try:
            request = json.loads(line)
            action = request["action"]
            if action == "isolation":
                checks = journey.isolation(request["host_port"])
            elif action in ("health", "signup", "persisted", "revoke", "unavailable", "denied_peer"):
                checks = getattr(journey, action)()
            elif action == "revoked":
                journey.session(401)
                checks = ["revocation-persists-after-restart"]
            else:
                raise CheckFailed("unknown-driver-command")
            print(json.dumps({"status": "PASS", "checks": checks}), flush=True)
        except CheckFailed as error:
            print(json.dumps({"status": "FAIL", "check": str(error)}), flush=True)
        except Exception as error:
            # Never print response/mail/password/credential or arbitrary exception text.
            print(json.dumps({"status": "FAIL", "check": "driver-" + type(error).__name__}), flush=True)


if __name__ == "__main__":
    main()
