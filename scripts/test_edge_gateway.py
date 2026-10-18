"""Exact gateway refusal partitions; optional pinned Caddy executes the real matchers.

Discovered by the repository gate. CANTOS_CADDY_BIN selects an already installed 2.11.7
binary; nothing is downloaded or provisioned. The only upstream is an in-process HTTP
recording double, never Kanidm. This is routing evidence, not issuer/browser acceptance.
"""
import contextlib
import http.client
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import os
from pathlib import Path
import signal
import socket
import subprocess
import tempfile
import threading
import time
import unittest

from test_edge_cache import CADDYFILE, block, lines, statements


def render(nodes):
    return "\n".join(line if body is None else line + " {\n" + render(body) + "\n}"
                     for line, body in nodes)


@contextlib.contextmanager
def gateway(binary, private):
    calls = []

    class Upstream(BaseHTTPRequestHandler):
        def handle_request(self):
            calls.append((self.command, self.path))
            self.rfile.read(int(self.headers.get("Content-Length", "0")))
            self.send_response(200)
            self.send_header("X-Cantos-Recording-Upstream", "yes")
            self.send_header("Content-Length", "0")
            self.end_headers()

        do_GET = do_HEAD = do_POST = do_PUT = do_PATCH = do_DELETE = do_OPTIONS = handle_request

        def log_message(self, *_):
            pass

    upstream = ThreadingHTTPServer(("127.0.0.1", 0), Upstream)
    thread = threading.Thread(target=upstream.serve_forever, daemon=True)
    thread.start()
    with socket.socket() as listener:
        listener.bind(("127.0.0.1", 0))
        port = listener.getsockname()[1]
    config = statements(CADDYFILE.read_text())
    identity = render(block(config, "{$CANTOS_IDM_HOST}"))
    policy = ("{\nadmin off\nauto_https off\n}\n"
              "(cantos_security) {\n" + render(block(config, "(cantos_security)")) + "\n}\n"
              "(cantos_kanidm_upstream) {\nreverse_proxy http://127.0.0.1:"
              + str(upstream.server_port) + "\n}\n"
              "http://127.0.0.1:" + str(port) + " {\n" + identity + "\n}\n")
    peer = "" if private is None else ("127.0.0.1/32" if private else "192.0.2.0/24")
    env = dict(os.environ, CANTOS_INTERNAL_CIDRS=peer)
    process = None
    try:
        with tempfile.TemporaryDirectory(prefix="cantos-gateway-") as directory:
            path = Path(directory) / "Caddyfile"
            path.write_text(policy)
            with (Path(directory) / "caddy.log").open("w+") as log:
                process = subprocess.Popen([binary, "run", "--config", str(path),
                                            "--adapter", "caddyfile"], env=env,
                                           stdout=log, stderr=log, start_new_session=True)
                deadline = time.monotonic() + 10
                while True:
                    try:
                        with socket.create_connection(("127.0.0.1", port), timeout=.1):
                            break
                    except OSError:
                        if process.poll() is not None or time.monotonic() > deadline:
                            log.seek(0)
                            raise AssertionError("Caddy fixture did not start: " + log.read())
                        time.sleep(.02)
                yield port, calls
    finally:
        if process and process.poll() is None:
            os.killpg(process.pid, signal.SIGTERM)
            try:
                process.wait(timeout=5)
            except subprocess.TimeoutExpired:
                os.killpg(process.pid, signal.SIGKILL)
                process.wait(timeout=5)
        upstream.shutdown()
        upstream.server_close()
        thread.join(timeout=5)


def request(port, method, path, headers=None):
    connection = http.client.HTTPConnection("127.0.0.1", port, timeout=3)
    try:
        connection.request(method, path, body=b"" if method == "POST" else None,
                           headers=headers or {})
        response = connection.getresponse()
        result = response.status, dict(response.getheaders())
        response.read()
        return result
    finally:
        connection.close()


class GatewayPolicyTests(unittest.TestCase):
    def test_backchannel_and_unknown_routes_have_no_public_matcher(self):
        identity = block(statements(CADDYFILE.read_text()), "{$CANTOS_IDM_HOST}")
        public = []
        for name in ("@kanidm_browser_get", "@kanidm_browser_post"):
            public.extend(path for line in lines(block(identity, name)) if line.startswith("path ")
                          for path in line.split()[1:])
        self.assertFalse(any("*" in path for path in public))
        for forbidden in ("/v1/auth", "/oauth2/token", "/oauth2/token/introspect",
                          "/oauth2/token/revoke", "/ui/admin", "/ui/api/admin",
                          "/ui/profile", "/ui/api/user_settings/edit_profile"):
            self.assertNotIn(forbidden, public)

    def test_upstream_keeps_private_ca_and_original_server_name(self):
        upstream = block(statements(CADDYFILE.read_text()), "(cantos_kanidm_upstream)")
        transport = block(block(upstream, "reverse_proxy https://cantos-kanidm:8443"),
                          "transport http")
        self.assertEqual(lines(transport), ["tls_trust_pool file /etc/caddy/internal-ca.pem",
                                           "tls_server_name cantos-kanidm"])


@unittest.skipUnless(os.environ.get("CANTOS_CADDY_BIN"), "optional installed pinned Caddy")
class GatewayCaddyTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.binary = str(Path(os.environ["CANTOS_CADDY_BIN"]).resolve())
        version = subprocess.run([cls.binary, "version"], capture_output=True,
                                 text=True, timeout=10, check=True)
        if not version.stdout.startswith("v2.11.7 "):
            raise AssertionError("fixture requires deployment pin Caddy2.11.7")

    def test_public_refusals_never_reach_upstream_even_with_spoofed_peer_headers(self):
        with gateway(self.binary, private=False) as (port, calls):
            paths = ("/ui/admin", "/ui/admin/person", "/ui/api/admin", "/ui/api/admin/group",
                     "/v1/person", "/v1/service_account", "/v1/group", "/v1/oauth2",
                     "/v1/system", "/v1/domain", "/v1/schema", "/v1/raw/search",
                     "/v1/recycle_bin", "/v1/sync_account", "/v1/application",
                     "/scim/v1/Entry", "/oauth2/token", "/oauth2/token/introspect",
                     "/oauth2/token/revoke", "/oauth2/openid/other/public_key.jwk",
                     "/.well-known/openid-configuration/oauth2/openid/other",
                     "/ui/api/user_settings/edit_profile", "/ui/reset/set_unixcred",
                     "/ui/reset/add_ssh_publickey", "/ui/api/delete_unixcred",
                     "/ui/api/remove_ssh_publickey", "/pkg/unknown.js", "/unknown")
            spoof = {"X-Forwarded-For": "127.0.0.1", "X-Real-IP": "127.0.0.1",
                     "Forwarded": "for=127.0.0.1"}
            for path in paths:
                for method in ("GET", "HEAD", "POST", "PUT", "PATCH", "DELETE", "OPTIONS", "TRACE"):
                    with self.subTest(path=path, method=method):
                        status, headers = request(port, method, path, spoof)
                        self.assertEqual(status, 404)
                        self.assertNotIn("X-Cantos-Recording-Upstream", headers)
                        self.assertEqual(headers.get("Cache-Control"), "no-store")
            self.assertEqual(calls, [])

    def test_exact_login_assets_and_credential_routes_allow_only_named_methods(self):
        with gateway(self.binary, private=False) as (port, calls):
            allowed = (("GET", "/ui/login"), ("GET", "/ui/oauth2?client_id=cantos-theatre-web-bff"),
                       ("GET", "/ui/login?state=a%2Fb"), ("GET", "/ui/reset?token=fixture"),
                       ("POST", "/ui/login/passkey"), ("POST", "/ui/login/pw"),
                       ("POST", "/ui/oauth2/consent"), ("POST", "/ui/api/cu_commit"),
                       ("GET", "/pkg/style.js?v=1"), ("HEAD", "/pkg/img/favicon.png"))
            for method, path in allowed:
                self.assertEqual(request(port, method, path)[0], 200, (method, path))
            self.assertEqual(calls, list(allowed))
            for method, path in (("GET", "/ui/api/cu_commit"), ("PUT", "/ui/login/pw"),
                                 ("POST", "/pkg/style.js"), ("OPTIONS", "/ui/login")):
                self.assertEqual(request(port, method, path)[0], 404)
            self.assertEqual(calls, list(allowed))

    def test_encoded_separators_dot_segments_and_unknown_suffixes_are_refused(self):
        with gateway(self.binary, private=False) as (port, calls):
            for path in ("/ui/%6cogin", "/ui%2flogin", "/ui%2Flogin", "/ui%5clogin",
                         "/ui//login", "/ui/./login", "/ui/x/../login", "/pkg/../ui/login",
                         "/ui/login/", "/ui/login/anything", "/ui/login;admin", "/ui\\login"):
                with self.subTest(path=path):
                    self.assertIn(request(port, "GET", path)[0], (400, 404))
            self.assertEqual(calls, [])

    def test_private_tcp_peer_retains_backchannel_and_management_access(self):
        with gateway(self.binary, private=True) as (port, calls):
            allowed = (("POST", "/oauth2/token"), ("POST", "/oauth2/token/introspect"),
                       ("POST", "/oauth2/token/revoke"), ("GET", "/v1/person"),
                       ("GET", "/ui/api/admin"),
                       ("GET", "/oauth2/openid/cantos-studio-web-bff/public_key.jwk"))
            for method, path in allowed:
                self.assertEqual(request(port, method, path)[0], 200)
            self.assertEqual(calls, list(allowed))

    def test_missing_private_ranges_cannot_admit_management(self):
        with gateway(self.binary, private=None) as (port, calls):
            self.assertEqual(request(port, "GET", "/v1/person")[0], 404)
            self.assertEqual(request(port, "GET", "/ui/login")[0], 200)
            self.assertEqual(calls, [("GET", "/ui/login")])
