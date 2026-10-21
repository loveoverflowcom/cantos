"""Bounded, read-only GHCR manifest/config observation for the release owner.

No image layers, build, publish, credential provisioning or activation. Tokens stay
in memory and are scoped to one exact repository; error codes contain no remote text.
"""

import base64
from dataclasses import dataclass
import hashlib
import http.client
import json
import os
from pathlib import Path
import re
import socket
import ssl
import urllib.error
import urllib.parse
import urllib.request

import release_artifact as contract
import release_oci as oci


MANIFEST_MEDIA = "application/vnd.oci.image.manifest.v1+json"
CONFIG_MEDIA = "application/vnd.oci.image.config.v1+json"
MAX_BYTES = 2 * 1024**2
MAX_AUTH_BYTES = 64 * 1024
CDN_ORIGIN = "https://pkg-containers.githubusercontent.com"


class ReadbackError(Exception):
    """Stable safe code only: never retain a response body, URL or credential."""


def require(condition, code):
    if not condition:
        raise ReadbackError(code)


def bounded_json(data, limit=MAX_BYTES):
    require(isinstance(data, bytes) and 0 < len(data) <= limit, "RESPONSE_SIZE")
    def unique(pairs):
        result = {}
        for key, value in pairs:
            require(key not in result, "JSON_DUPLICATE_KEY")
            result[key] = value
        return result
    def reject_constant(_):
        raise ReadbackError("JSON_NONFINITE")
    try:
        value = json.loads(data, object_pairs_hook=unique, parse_constant=reject_constant)
    except (ValueError, UnicodeError, RecursionError):
        raise ReadbackError("JSON_INVALID") from None
    require(isinstance(value, dict), "JSON_OBJECT_REQUIRED")
    return value


@dataclass(frozen=True)
class Response:
    status: int
    headers: dict
    body: bytes


class NoRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, request, fp, code, message, headers, newurl):
        return None


class HttpsTransport:
    """TLS verification stays enabled; redirects are decisions in the GHCR adapter."""

    def __init__(self):
        self.opener = urllib.request.build_opener(
            NoRedirect(), urllib.request.HTTPSHandler(context=ssl.create_default_context()))

    def __call__(self, url, headers, limit):
        request = urllib.request.Request(url, headers=headers, method="GET")
        try:
            response = self.opener.open(request, timeout=10)
        except urllib.error.HTTPError as error:
            response = error
        except urllib.error.URLError as error:
            if isinstance(error.reason, (ssl.SSLError, ssl.CertificateError)):
                raise ReadbackError("TLS_VERIFICATION") from None
            if isinstance(error.reason, (TimeoutError, socket.timeout)):
                raise ReadbackError("NETWORK_TIMEOUT") from None
            if isinstance(error.reason, socket.gaierror):
                raise ReadbackError("NETWORK_DNS") from None
            raise ReadbackError("NETWORK_UNAVAILABLE") from None
        except (TimeoutError, socket.timeout):
            raise ReadbackError("NETWORK_TIMEOUT") from None
        except ssl.SSLError:
            raise ReadbackError("TLS_VERIFICATION") from None
        except http.client.HTTPException:
            raise ReadbackError("NETWORK_PROTOCOL") from None
        except OSError:
            raise ReadbackError("NETWORK_UNAVAILABLE") from None
        try:
            with response:
                selected = {}
                for key in ("content-type", "content-length", "docker-content-digest", "www-authenticate", "location"):
                    values = response.headers.get_all(key, [])
                    require(len(values) <= 1, "HTTP_AMBIGUOUS_HEADERS")
                    if values:
                        require(len(values[0]) <= 8192, "HTTP_HEADER_SIZE")
                        selected[key] = values[0]
                declared = selected.get("content-length")
                if response.code == 200 and declared is not None:
                    require(re.fullmatch(r"[0-9]{1,10}", declared) and int(declared) <= limit, "RESPONSE_SIZE")
                body = response.read(limit + 1) if response.code == 200 else b""
                require(len(body) <= limit, "RESPONSE_SIZE")
                if response.code == 200 and declared is not None:
                    require(len(body) == int(declared), "NETWORK_TRUNCATED")
                return Response(response.code, selected, body)
        except (TimeoutError, socket.timeout):
            raise ReadbackError("NETWORK_TIMEOUT") from None
        except ssl.SSLError:
            raise ReadbackError("TLS_VERIFICATION") from None
        except http.client.HTTPException:
            raise ReadbackError("NETWORK_READ") from None
        except (OSError, ValueError):
            raise ReadbackError("NETWORK_READ") from None


def load_authfile(path):
    """Read only the explicitly named private authfile; never run credential helpers."""
    if path is None:
        return None
    try:
        before = contract.regular(Path(path), MAX_AUTH_BYTES)
        require(before.st_uid == os.getuid() and before.st_mode & 0o077 == 0, "AUTHFILE_PERMISSIONS")
        with Path(path).open("rb") as stream:
            data = stream.read(MAX_AUTH_BYTES + 1)
        after = Path(path).stat()
        require((before.st_dev, before.st_ino, before.st_size, before.st_mtime_ns, before.st_ctime_ns) ==
                (after.st_dev, after.st_ino, after.st_size, after.st_mtime_ns, after.st_ctime_ns), "AUTHFILE_CHANGED")
        value = bounded_json(data, MAX_AUTH_BYTES)
        require(not value.get("credsStore") and not value.get("credHelpers"), "AUTHFILE_HELPER_UNSUPPORTED")
        auth = value.get("auths", {}).get("ghcr.io", {}).get("auth")
        require(isinstance(auth, str) and len(auth) <= 32768, "AUTHFILE_GHCR_ENTRY")
        decoded = base64.b64decode(auth, validate=True).decode("ascii")
        username, separator, password = decoded.partition(":")
        require(separator and re.fullmatch(r"[A-Za-z0-9-]{1,100}", username) and password and
                all(32 < ord(c) < 127 for c in password), "AUTHFILE_CREDENTIAL")
        return "Basic " + auth
    except (ValueError, OSError, UnicodeError, AttributeError):
        raise ReadbackError("AUTHFILE_INVALID") from None


def http_ok(response):
    if response.status != 200:
        require(type(response.status) is int and 100 <= response.status <= 599, "HTTP_STATUS_INVALID")
        raise ReadbackError("HTTP_" + str(response.status))
    require(len(response.body) <= MAX_BYTES, "RESPONSE_SIZE")


def media(response, allowed):
    require(response.headers.get("content-type", "").split(";", 1)[0].strip().lower() in allowed, "MEDIA_TYPE")


def check_digest(data, expected):
    require("sha256:" + hashlib.sha256(data).hexdigest() == expected, "DIGEST_MISMATCH")


def descriptor(value, expected_media=None):
    require(isinstance(value, dict), "DESCRIPTOR_SHAPE")
    require(isinstance(value.get("digest"), str) and
            re.fullmatch(r"sha256:[0-9a-f]{64}", value["digest"]), "DESCRIPTOR_DIGEST")
    require(type(value.get("size")) is int and 0 < value["size"] <= 2**63 - 1, "DESCRIPTOR_SIZE")
    if expected_media:
        require(value.get("mediaType") == expected_media, "MEDIA_TYPE")
    return value


def verify_bytes(service, image, manifest_response, config_response, source_sha, configuration_digest, platform):
    """Pure byte/identity checks; cannot establish executable contents or runtime qualification."""
    http_ok(manifest_response)
    http_ok(config_response)
    media(manifest_response, {MANIFEST_MEDIA})
    registry_digest = image["ref"].split("@", 1)[1]
    check_digest(manifest_response.body, registry_digest)
    for response, expected in ((manifest_response, registry_digest), (config_response, image["config_digest"])):
        header = response.headers.get("docker-content-digest")
        require(header is None or header == expected, "DIGEST_HEADER_MISMATCH")
    document = bounded_json(manifest_response.body)
    require(type(document.get("schemaVersion")) is int and document["schemaVersion"] == 2 and
            document.get("mediaType") == MANIFEST_MEDIA and not document.get("artifactType") and
            not document.get("subject"), "MANIFEST_SHAPE")
    config = descriptor(document.get("config"), CONFIG_MEDIA)
    require(config["digest"] == image["config_digest"] and config["size"] <= MAX_BYTES, "CONFIG_DESCRIPTOR")
    layers = document.get("layers")
    require(isinstance(layers, list) and 1 <= len(layers) <= 256, "LAYERS_SHAPE")
    for layer in layers:
        descriptor(layer)
        require(layer.get("mediaType") in {"application/vnd.oci.image.layer.v1.tar",
                "application/vnd.oci.image.layer.v1.tar+gzip", "application/vnd.oci.image.layer.v1.tar+zstd"} and
                not layer.get("urls"), "LAYER_MEDIA_OR_FOREIGN_URL")
    media(config_response, {CONFIG_MEDIA, "application/octet-stream"})
    require(len(config_response.body) == config["size"], "CONFIG_SIZE")
    check_digest(config_response.body, image["config_digest"])
    document = bounded_json(config_response.body)
    require(isinstance(document.get("os"), str) and isinstance(document.get("architecture"), str) and
            document["os"] + "/" + document["architecture"] == platform and
            not document.get("variant"), "PLATFORM_MISMATCH")
    rootfs = document.get("rootfs")
    require(isinstance(rootfs, dict) and rootfs.get("type") == "layers" and
            isinstance(rootfs.get("diff_ids"), list) and len(rootfs["diff_ids"]) == len(layers) and
            all(isinstance(v, str) and re.fullmatch(r"sha256:[0-9a-f]{64}", v) for v in rootfs["diff_ids"]),
            "CONFIG_ROOTFS")
    settings = document.get("config")
    require(isinstance(settings, dict), "CONFIG_SHAPE")
    labels = settings.get("Labels")
    require(isinstance(labels, dict) and labels.get("org.opencontainers.image.source") == contract.REPOSITORY and
            labels.get("org.opencontainers.image.revision") == source_sha and
            labels.get("io.cantos.release.configuration") == configuration_digest and
            labels.get("io.cantos.binary") == service, "IMAGE_IDENTITY_MISMATCH")
    user = settings.get("User")
    require(isinstance(user, str) and re.fullmatch(r"[1-9][0-9]{0,9}:[1-9][0-9]{0,9}", user) and
            all(int(v) < 2**32 - 1 for v in user.split(":")), "IMAGE_USER_MUST_BE_NONROOT")
    require(settings.get("Entrypoint") == ["/usr/local/bin/" + service] and
            settings.get("Cmd") in (None, []), "IMAGE_ENTRYPOINT")
    return {"ref": image["ref"], "config_digest": image["config_digest"], "platform": platform,
            "user": user, "binary": service, "layers": "NOT_RUN"}


class GhcrClient:
    def __init__(self, service, transport, basic=None):
        require(service in oci.SERVICES, "SERVICE_SCOPE")
        self.repository = "loveoverflowcom/" + service
        self.transport, self.basic, self.token = transport, basic, None

    def _token(self, challenge):
        require(isinstance(challenge, str) and len(challenge) <= 4096 and challenge[:7].lower() == "bearer ", "AUTH_CHALLENGE")
        parts = challenge[7:].split(",")
        fields = {}
        for part in parts:
            match = re.fullmatch(r'\s*(realm|service|scope)="([^"\\\r\n]*)"\s*', part, flags=re.IGNORECASE)
            require(match is not None and match[1].lower() not in fields, "AUTH_CHALLENGE")
            fields[match[1].lower()] = match[2]
        scope = "repository:" + self.repository + ":pull"
        require(fields == {"realm": "https://ghcr.io/token", "service": "ghcr.io", "scope": scope}, "AUTH_CHALLENGE_SCOPE")
        url = "https://ghcr.io/token?" + urllib.parse.urlencode({"service": "ghcr.io", "scope": scope})
        headers = {"Accept": "application/json"}
        if self.basic:
            headers["Authorization"] = self.basic
        response = self.transport(url, headers, MAX_AUTH_BYTES)
        http_ok(response)
        media(response, {"application/json"})
        value = bounded_json(response.body, MAX_AUTH_BYTES)
        token = value.get("token", value.get("access_token"))
        require(isinstance(token, str) and re.fullmatch(r"[A-Za-z0-9._~+/=-]{1,32768}", token) and
                value.get("access_token", token) == token, "AUTH_TOKEN_INVALID")
        self.token = token

    def get(self, kind, digest):
        require(kind in ("manifests", "blobs") and re.fullmatch(r"sha256:[0-9a-f]{64}", digest), "REQUEST_SCOPE")
        url = "https://ghcr.io/v2/" + self.repository + "/" + kind + "/" + digest
        headers = {"Accept": MANIFEST_MEDIA if kind == "manifests" else CONFIG_MEDIA}
        if self.token:
            headers["Authorization"] = "Bearer " + self.token
        response = self.transport(url, headers, MAX_BYTES)
        if response.status == 401:
            self._token(response.headers.get("www-authenticate"))
            headers["Authorization"] = "Bearer " + self.token
            response = self.transport(url, headers, MAX_BYTES)
        for _ in range(2):
            if response.status not in (301, 302, 303, 307, 308):
                break
            require(kind == "blobs", "REDIRECT_FORBIDDEN")
            location = response.headers.get("location", "")
            try:
                parsed = urllib.parse.urlsplit(location)
            except ValueError:
                raise ReadbackError("REDIRECT_FORBIDDEN") from None
            require(len(location) <= 8192 and parsed.scheme + "://" + parsed.netloc == CDN_ORIGIN and not parsed.fragment and
                    all(33 <= ord(c) < 127 for c in location), "REDIRECT_FORBIDDEN")
            # CDN signed URLs are opaque and never logged; no Basic/Bearer/cookie crosses origins.
            response = self.transport(location, {"Accept": "application/octet-stream"}, MAX_BYTES)
        require(response.status not in (301, 302, 303, 307, 308), "REDIRECT_LIMIT")
        http_ok(response)
        return response


def observe(manifest, plan, *, authfile=None, transport=None):
    """Called only after offline admission/bundle/evidence verification by the release shell."""
    basic = load_authfile(authfile)
    transport = transport if transport is not None else HttpsTransport()
    observations = {}
    for service in oci.SERVICES:
        image = manifest["images"][service]
        client = GhcrClient(service, transport, basic)
        raw_manifest = client.get("manifests", image["ref"].split("@", 1)[1])
        # Refuse a wrong manifest/config descriptor before requesting a blob.
        media(raw_manifest, {MANIFEST_MEDIA})
        check_digest(raw_manifest.body, image["ref"].split("@", 1)[1])
        descriptor_value = descriptor(bounded_json(raw_manifest.body).get("config"), CONFIG_MEDIA)
        require(descriptor_value["digest"] == image["config_digest"] and descriptor_value["size"] <= MAX_BYTES,
                "CONFIG_DESCRIPTOR")
        raw_config = client.get("blobs", image["config_digest"])
        observations[service] = verify_bytes(service, image, raw_manifest, raw_config, manifest["source_sha"],
                                             manifest["configuration_sha256"], manifest["platform"])
    return {**plan, "kind": "cantos-native-oci-registry-observation", "operation": "OBSERVED",
            "images": observations, "release_eligible": False,
            "gates": {**plan["gates"], "registry-access": "PASS", "registry-manifest-config": "PASS",
                      "image-layers": "NOT_RUN", "image-pair": "NOT_RUN"}}
