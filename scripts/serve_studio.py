#!/usr/bin/env python3
"""Serve the API against ONLY the disposable cluster retained by test_postgres.py.

`serve_studio.py tls-front` is its companion: a loopback TLS front end for that API, so the
HTTPS media URL the API advertises is one a browser can actually open (see `tls_front`).
"""
import http.client
import http.server
import json
import os
import re
import shutil
import socketserver
import ssl
import subprocess
import sys
from pathlib import Path
from urllib.parse import urlparse

from retained_postgres import OWNERSHIP_RECORD, confirm_running, validate_ownership

ROOT = Path(__file__).resolve().parents[1]
SEED_RECORD = "target/revision-evidence/theatre-seed.json"
API_ADDRESS = ("127.0.0.1", 8080)
# The API admits only an HTTPS media origin and itself speaks plain HTTP on 8080, so the origin a
# listener's browser follows must be a TLS front end on another loopback port.
DEFAULT_MEDIA_ORIGIN = "https://localhost:8443"
LOCAL_TLS_DIRECTORY = "target/local-tls"


def local_media_origin(inherited):
    """The exact HTTPS origin the local TLS front end serves; never the API's own HTTP port."""
    origin = inherited.get("CANTOS_LOCAL_MEDIA_ORIGIN", DEFAULT_MEDIA_ORIGIN)
    parsed = urlparse(origin)
    try:
        port = parsed.port
    except ValueError:
        port = None
    if (parsed.scheme != "https" or parsed.hostname not in ("localhost", "127.0.0.1")
            or port is None or port == API_ADDRESS[1] or origin != f"https://{parsed.hostname}:{port}"):
        raise ValueError(
            "CANTOS_LOCAL_MEDIA_ORIGIN must be exactly https://localhost:<port> or "
            f"https://127.0.0.1:<port>, a TLS front end for the loopback API, not its HTTP port {API_ADDRESS[1]}")
    return origin


def theatre_seed_url(config, seed):
    """The seeded app-role URL only when `cargo xtask seed theatre` recorded it for this cluster."""
    if not isinstance(seed, dict) or seed.get("cluster") != config.get("cluster") or seed.get("port") != config.get("port"):
        return None
    database = seed.get("database")
    if not isinstance(database, str) or not re.fullmatch(r"cantos_test_[0-9a-f]{32}", database):
        raise ValueError("unsafe Theatre seed record; run `cargo xtask seed theatre --reset`")
    return f"postgresql://cantos_app@127.0.0.1:{config['port']}/{database}"


def theatre_seed_media(config, seed, media_origin=DEFAULT_MEDIA_ORIGIN):
    """Only retained seed-owned storage may enable media on the disposable API host."""
    fields = ("private_root", "publication_root", "playback_key_file")
    if not any(field in seed for field in fields):
        return {}  # Older catalog-only receipts leave public delivery unavailable.
    if not all(isinstance(seed.get(field), str) for field in fields):
        raise ValueError("incomplete Theatre seed media receipt; seed again")
    cluster = Path(config["cluster"]).resolve()
    paths = [Path(seed[field]) for field in fields]
    if any(not path.is_absolute() or path.is_symlink() or not path.exists()
           or not path.resolve().is_relative_to(cluster) for path in paths):
        raise ValueError("Theatre seed media must remain within its retained cluster")
    private, publication, key = paths
    if (private.parent != publication.parent or key.parent != private.parent
            or not private.parent.name.startswith("theatre-seed-")
            or private.name != "private" or publication.name != "publication" or key.name != "playback.key"):
        raise ValueError("unexpected Theatre seed media layout")
    return {"CANTOS_AUDIO_ROOT": str(private), "CANTOS_PUBLICATION_ROOT": str(publication),
            "CANTOS_PLAYBACK_KEY_FILE": str(key), "CANTOS_PUBLIC_MEDIA_ORIGIN": media_origin}


def studio_environment(config, inherited, seed=None):
    """Select only a database emitted by the fresh disposable runner, never an inherited URL."""
    context = inherited.get("CANTOS_TEST_CONTEXT", "editorial")
    fields = {"editorial": "app_url", "catalog": "catalog_app_url", "audio": "audio_app_url",
              "theatre": "theatre_app_url"}
    if context not in fields:
        raise ValueError("test context must be editorial, catalog, audio or theatre")
    seeded = theatre_seed_url(config, seed) if context == "theatre" else None
    if config.get("purpose") == "retained-disposable-cluster":
        if context != "theatre" or config.get("retained") is not True or seeded is None:
            raise ValueError("retained cluster ownership requires a matching Theatre seed receipt")
    elif context == "theatre" and (config.get("retained") is not True or "theatre_app_url" not in config):
        raise ValueError("Theatre requires a retained test fixture; run `cargo xtask verify postgres --keep` again")
    app_url = seeded or config[fields[context]]
    url = urlparse(app_url)
    if (url.scheme != "postgresql" or url.hostname != "127.0.0.1"
            or url.username != "cantos_app" or url.password is not None
            or not url.path.startswith("/cantos_test_") or url.query or url.fragment):
        raise ValueError("expected disposable test database")
    env = dict(inherited, CANTOS_ENV="development", CANTOS_BIND_ADDRESS="127.0.0.1:8080", DATABASE_URL=app_url)
    if context == "theatre":
        for name in ("CANTOS_STORAGE_CONFIG", "CANTOS_AUDIO_ROOT", "CANTOS_PUBLICATION_ROOT",
                     "CANTOS_PUBLIC_MEDIA_ORIGIN", "CANTOS_PLAYBACK_KEY_FILE"):
            env.pop(name, None)
        if seeded:
            env.update(theatre_seed_media(config, seed, local_media_origin(inherited)))
    if context == "audio":
        root = Path(config["audio_root"])
        if not root.is_absolute():
            raise ValueError("expected absolute private test audio root")
        env["CANTOS_AUDIO_ROOT"] = str(root)
    return env


def serving_config(context, seed):
    """Ownership proves one seeded Theatre target, never the full runner's other fixtures."""
    ownership_path = ROOT / OWNERSHIP_RECORD
    if context == "theatre" and seed is not None and ownership_path.is_file():
        ownership = json.loads(ownership_path.read_text())
        if not isinstance(ownership, dict):
            raise ValueError("invalid retained cluster ownership record")
        if (not isinstance(seed, dict) or seed.get("cluster") != ownership.get("cluster")
                or seed.get("port") != ownership.get("port")):
            raise ValueError("Theatre seed does not match retained cluster ownership; seed again")
        validate_ownership(ownership, ROOT / "target/revision-evidence")
        return ownership
    return json.loads((ROOT / "target/revision-evidence/run.json").read_text())


HOP_BY_HOP = {"connection", "keep-alive", "proxy-authenticate", "proxy-authorization", "te",
              "trailers", "transfer-encoding", "upgrade", "host"}
MAX_FORWARDED_BODY = 32 * 1024


def redact(path):
    """A media path carries a bearer grant: it is never logged."""
    return re.sub(r"(/media/)[^/?#]+", r"\1<redacted>", path)


class TlsFront(http.server.BaseHTTPRequestHandler):
    """Forward `/api/` to the loopback API byte for byte, including Range requests and 206s."""
    protocol_version = "HTTP/1.1"
    upstream = API_ADDRESS

    def log_message(self, format, *args):  # noqa: A002 - signature fixed by the base class
        sys.stderr.write("tls-front %s %s\n" % (self.address_string(), redact(format % args)))

    def refuse(self, status, code, retryable):
        body = json.dumps({"code": code, "retryable": retryable}).encode()
        self.send_response(status)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body)))
        self.send_header("Connection", "close")
        self.end_headers()
        if self.command != "HEAD":
            self.wfile.write(body)
        self.close_connection = True

    def forward(self):
        if not self.path.startswith("/api/"):
            return self.refuse(404, "not_found", False)
        try:
            length = int(self.headers.get("Content-Length") or 0)
        except ValueError:
            return self.refuse(400, "invalid_request", False)
        if not 0 <= length <= MAX_FORWARDED_BODY:
            return self.refuse(400, "invalid_request", False)
        body = self.rfile.read(length) if length else None
        headers = {key: value for key, value in self.headers.items() if key.lower() not in HOP_BY_HOP}
        try:
            connection = http.client.HTTPConnection(*self.upstream, timeout=30)
            connection.request(self.command, self.path, body=body, headers=headers)
            response = connection.getresponse()
        except OSError:
            return self.refuse(503, "unavailable", True)
        try:
            self.send_response_only(response.status, response.reason)
            for key, value in response.getheaders():
                if key.lower() not in HOP_BY_HOP:
                    self.send_header(key, value)
            self.send_header("Connection", "close")
            self.end_headers()
            if self.command != "HEAD":
                while chunk := response.read(65536):
                    self.wfile.write(chunk)
        except OSError:
            pass  # The listener went away mid-stream, as a seeking player does.
        finally:
            connection.close()
            self.close_connection = True

    do_GET = do_HEAD = do_POST = do_OPTIONS = forward


class TlsFrontServer(http.server.ThreadingHTTPServer):
    daemon_threads = True

    def __init__(self, address, context):
        super().__init__(address, TlsFront)
        self.context = context

    def finish_request(self, request, client_address):
        # Handshake in the connection's own thread so a silent preconnect cannot stall accepts.
        try:
            secured = self.context.wrap_socket(request, server_side=True)
        except (ssl.SSLError, OSError):
            return
        try:
            super().finish_request(secured, client_address)
        finally:
            secured.close()


def generate_certificate(directory):
    """A short-lived self-signed certificate for localhost; the operator decides about trust."""
    directory.mkdir(parents=True, exist_ok=True)
    certificate, key = directory / "localhost.pem", directory / "localhost-key.pem"
    subprocess.run(
        ["openssl", "req", "-x509", "-newkey", "rsa:2048", "-nodes", "-days", "30",
         "-keyout", str(key), "-out", str(certificate), "-subj", "/CN=localhost",
         "-addext", "subjectAltName=DNS:localhost,IP:127.0.0.1"],
        check=True, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    key.chmod(0o600)
    return certificate, key


def tls_front(arguments, inherited):
    origin = urlparse(local_media_origin(inherited))
    directory = ROOT / LOCAL_TLS_DIRECTORY
    certificate = Path(inherited.get("CANTOS_LOCAL_TLS_CERT", directory / "localhost.pem"))
    key = Path(inherited.get("CANTOS_LOCAL_TLS_KEY", directory / "localhost-key.pem"))
    if "--generate" in arguments and not (certificate.is_file() and key.is_file()):
        if shutil.which("openssl") is None:
            raise ValueError("--generate needs openssl on PATH")
        certificate, key = generate_certificate(directory)
    if not (certificate.is_file() and key.is_file()):
        raise ValueError(f"missing {certificate} or {key}; rerun with --generate or set "
                         "CANTOS_LOCAL_TLS_CERT and CANTOS_LOCAL_TLS_KEY")
    context = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
    context.load_cert_chain(certificate, key)
    server = TlsFrontServer((origin.hostname, origin.port), context)
    print(f"TLS front end for the loopback API: https://{origin.hostname}:{origin.port}/api/ -> "
          f"http://{API_ADDRESS[0]}:{API_ADDRESS[1]}/api/ (certificate {certificate}). A browser reaches "
          "media only after you trust or accept this certificate yourself; this script edits no trust "
          "store.", flush=True)
    try:
        server.serve_forever()
    except KeyboardInterrupt:
        pass
    finally:
        server.server_close()


def main():
    context = os.environ.get("CANTOS_TEST_CONTEXT", "editorial")
    seed_path = ROOT / SEED_RECORD
    seed = None
    if context == "theatre" and seed_path.is_file():
        seed = json.loads(seed_path.read_text())
        if not isinstance(seed, dict):
            raise ValueError("invalid Theatre seed receipt")
    config = serving_config(context, seed)
    env = studio_environment(config, os.environ, seed)
    if config.get("purpose") == "retained-disposable-cluster":
        confirm_running(config, ROOT / "target/revision-evidence", os.environ)
    os.chdir(ROOT)
    if context == "theatre" and theatre_seed_url(config, seed) is not None:
        public = [work for work in seed["works"] if work["fate"] == "public"]
        print(f"Theatre seed catalog: {len(public)} public works from `cargo xtask seed theatre` "
              f"(rights valid until {seed['rights_valid_until_ms']} ms); "
              "identity and native playback remain separate.", flush=True)
        if "CANTOS_PUBLICATION_ROOT" in env:
            origin = env["CANTOS_PUBLIC_MEDIA_ORIGIN"]
            print(f"Local publication storage enabled. Grant URLs name {origin}; a browser reaches them only "
                  "through the TLS front end: `python3 scripts/serve_studio.py tls-front --generate` "
                  "(see docs/deployment/theatre-seeded-catalog.md).", flush=True)
    elif context == "theatre":
        if seed is not None:
            print("Theatre seed record belongs to another cluster; ignored.", flush=True)
        print("Theatre test fixture: empty public catalog; identity and playback remain separate.", flush=True)
        print("Seed synthetic works with `cargo xtask seed theatre`.", flush=True)
    else:
        print(f"Test script ID: {config['script']}", flush=True)
        print("Sign in with the synthetic alice test token (64 lowercase a characters).", flush=True)
    binary = str(ROOT / "target/debug/cantos-server")
    os.execve(binary, [binary], env)


if __name__ == "__main__":
    if sys.argv[1:2] == ["tls-front"]:
        tls_front(sys.argv[2:], os.environ)
    else:
        main()
