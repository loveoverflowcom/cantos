"""Production readback parser/client with an explicit HTTP transport double; no GHCR calls."""

import base64
import hashlib
import http.client
import io
import json
from pathlib import Path
import socket
import ssl
import tempfile
import unittest
from unittest.mock import patch
import urllib.error

import release_artifact as contract
import release_oci as oci
import release_oci_registry as registry
from test_release_oci import CONFIG, fixture


def raw(value):
    # The registry digest binds these actual bytes, not a canonicalized re-encoding.
    return json.dumps(value, indent=2).encode()


def sha(data):
    return "sha256:" + hashlib.sha256(data).hexdigest()


def image_fixture(service="cantos-server", source="a" * 40, *, config_change=None, manifest_change=None):
    config = {"os": "linux", "architecture": "amd64",
              "rootfs": {"type": "layers", "diff_ids": ["sha256:" + "d" * 64]}, "config": {
        "User": "10001:10001", "Entrypoint": ["/usr/local/bin/" + service], "Cmd": [],
        "Labels": {"org.opencontainers.image.source": contract.REPOSITORY,
                   "org.opencontainers.image.revision": source,
                   "io.cantos.release.configuration": CONFIG, "io.cantos.binary": service}}}
    if config_change:
        config_change(config)
    config_data = raw(config)
    manifest = {"schemaVersion": 2, "mediaType": registry.MANIFEST_MEDIA,
                "config": {"mediaType": registry.CONFIG_MEDIA, "size": len(config_data), "digest": sha(config_data)},
                "layers": [{"mediaType": "application/vnd.oci.image.layer.v1.tar+gzip", "size": 1,
                            "digest": "sha256:" + "f" * 64}]}
    if manifest_change:
        manifest_change(manifest)
    manifest_data = raw(manifest)
    image = {"ref": "ghcr.io/loveoverflowcom/" + service + "@" + sha(manifest_data), "config_digest": sha(config_data)}
    return image, registry.Response(200, {"content-type": registry.MANIFEST_MEDIA}, manifest_data), \
        registry.Response(200, {"content-type": "application/octet-stream"}, config_data)


class RecordingTransport:
    def __init__(self, responses):
        self.responses = list(responses)
        self.calls = []

    def __call__(self, url, headers, limit):
        self.calls.append((url, dict(headers), limit))
        if not self.responses:
            raise AssertionError("unexpected request")
        return self.responses.pop(0)


def bind(root):
    source, args, manifest, admission = fixture(root)
    responses = []
    for service in oci.SERVICES:
        image, first, second = image_fixture(service, args.source_sha)
        manifest["images"][service] = image
        responses.extend((first, second))
    evidence_path = args.manifest.parent / "evidence.json"
    evidence = contract.read_json(evidence_path)
    evidence["images"] = manifest["images"]
    evidence_path.write_bytes(raw(evidence))
    manifest["evidence"] = contract.file_record(evidence_path, evidence_path.parent)
    args.manifest.write_bytes(raw(manifest))
    args.manifest_digest = contract.file_record(args.manifest, args.manifest.parent)["sha256"]
    admission["manifest_sha256"] = args.manifest_digest
    args.admission.write_bytes(raw(admission))
    args.admission_digest = contract.file_record(args.admission, args.admission.parent)["sha256"]
    args.registry_readback, args.authfile = True, None
    return source, args, manifest, responses


class RegistryReadbackTests(unittest.TestCase):
    def test_pair_flow_verifies_actual_bytes_with_external_http_double_and_never_qualifies_a_release(self):
        with tempfile.TemporaryDirectory() as temporary:
            source, args, manifest, responses = bind(Path(temporary))
            transport = RecordingTransport(responses)
            with patch.object(registry, "HttpsTransport", return_value=transport):
                result = oci.execute(source, args)
            self.assertEqual(result["operation"], "OBSERVED")
            self.assertFalse(result["release_eligible"])
            self.assertEqual(result["gates"]["registry-manifest-config"], "PASS")
            for gate in ("native-qualification", "activation", "cleanup", "image-layers", "image-pair"):
                self.assertEqual(result["gates"][gate], "NOT_RUN")
            self.assertEqual(len(transport.calls), 4)
            for i, service in enumerate(oci.SERVICES):
                item = manifest["images"][service]
                prefix = "https://ghcr.io/v2/loveoverflowcom/" + service
                self.assertEqual(transport.calls[i * 2][0], prefix + "/manifests/" + item["ref"].split("@")[1])
                self.assertEqual(transport.calls[i * 2 + 1][0], prefix + "/blobs/" + item["config_digest"])
                self.assertNotIn("Authorization", transport.calls[i * 2][1])

    def test_dry_run_and_missing_qualification_stop_before_authfile_or_http_effects(self):
        with tempfile.TemporaryDirectory() as temporary:
            source, args, manifest, _ = bind(Path(temporary))
            args.dry_run, args.authfile = True, Path(temporary) / "missing-authfile"
            with patch.object(registry, "observe", side_effect=AssertionError("unexpected remote/auth effect")):
                self.assertEqual(oci.execute(source, args)["operation"], "PLAN_ONLY")
                args.dry_run = False
                args.source_sha = "a" * 40
                with self.assertRaisesRegex(ValueError, "source SHA mismatch"):
                    oci.execute(source, args)
            args.source_sha = manifest["source_sha"]
            evidence_path = args.manifest.parent / "evidence.json"
            evidence = contract.read_json(evidence_path)
            evidence["gates"]["native-e2e"]["status"] = "NOT_RUN"
            evidence_path.write_bytes(raw(evidence))
            manifest["evidence"] = contract.file_record(evidence_path, evidence_path.parent)
            args.manifest.write_bytes(raw(manifest))
            args.manifest_digest = contract.file_record(args.manifest, args.manifest.parent)["sha256"]
            admission = contract.read_json(args.admission)
            admission["manifest_sha256"] = args.manifest_digest
            args.admission.write_bytes(raw(admission))
            args.admission_digest = contract.file_record(args.admission, args.admission.parent)["sha256"]
            with patch.object(registry, "observe", side_effect=AssertionError("unexpected remote/auth effect")):
                with self.assertRaisesRegex(ValueError, "producer gate did not PASS: native-e2e"):
                    oci.execute(source, args)

    def verify(self, values):
        image, first, second = values
        return registry.verify_bytes("cantos-server", image, first, second, "a" * 40, CONFIG, "linux/amd64")

    def test_digest_verifies_raw_bytes_and_header_not_parsed_json_or_config_id(self):
        image, first, second = image_fixture()
        for changed, expected in (
            ((image, registry.Response(200, first.headers, first.body + b" "), second), "DIGEST_MISMATCH"),
            ((image, first, registry.Response(200, second.headers, second.body + b" ")), "CONFIG_SIZE"),
            ((image, registry.Response(200, {**first.headers, "docker-content-digest": image["config_digest"]}, first.body), second), "DIGEST_HEADER_MISMATCH")):
            with self.subTest(expected=expected), self.assertRaisesRegex(registry.ReadbackError, "^" + expected + "$"):
                self.verify(changed)

    def test_platform_labels_user_and_binary_checks_are_independent_of_self_declared_image_refs(self):
        cases = ((lambda c: c.update(os="windows"), "PLATFORM_MISMATCH"),
                 (lambda c: c.update(architecture="arm64"), "PLATFORM_MISMATCH"),
                 (lambda c: c.pop("os"), "PLATFORM_MISMATCH"),
                 (lambda c: c.pop("rootfs"), "CONFIG_ROOTFS"),
                 (lambda c: c["rootfs"].update(diff_ids=[]), "CONFIG_ROOTFS"),
                 (lambda c: c["config"]["Labels"].update({"org.opencontainers.image.revision": "b" * 40}), "IMAGE_IDENTITY_MISMATCH"),
                 (lambda c: c["config"]["Labels"].update({"io.cantos.release.configuration": "b" * 64}), "IMAGE_IDENTITY_MISMATCH"),
                 (lambda c: c["config"]["Labels"].update({"org.opencontainers.image.source": "https://other.invalid"}), "IMAGE_IDENTITY_MISMATCH"),
                 (lambda c: c["config"]["Labels"].update({"io.cantos.binary": "cantos-identity"}), "IMAGE_IDENTITY_MISMATCH"),
                 (lambda c: c["config"].update(User="0:10001"), "IMAGE_USER_MUST_BE_NONROOT"),
                 (lambda c: c["config"].update(User="10001:0"), "IMAGE_USER_MUST_BE_NONROOT"),
                 (lambda c: c["config"].update(User="root"), "IMAGE_USER_MUST_BE_NONROOT"),
                 (lambda c: c["config"].update(Entrypoint=["/bin/sh"]), "IMAGE_ENTRYPOINT"),
                 (lambda c: c["config"].update(Cmd=["--mock"]), "IMAGE_ENTRYPOINT"))
        for change, expected in cases:
            with self.subTest(expected=expected), self.assertRaisesRegex(registry.ReadbackError, "^" + expected + "$"):
                self.verify(image_fixture(config_change=change))

    def test_index_wrong_media_descriptor_and_foreign_layers_refuse(self):
        cases = ((lambda m: m.update(schemaVersion=True), "MANIFEST_SHAPE"),
                 (lambda m: m.update(mediaType="application/vnd.oci.image.index.v1+json"), "MANIFEST_SHAPE"),
                 (lambda m: m["config"].update(mediaType="application/json"), "MEDIA_TYPE"),
                 (lambda m: m["config"].update(size=registry.MAX_BYTES + 1), "CONFIG_DESCRIPTOR"),
                 (lambda m: m["config"].update(digest="sha256:" + "e" * 64), "CONFIG_DESCRIPTOR"),
                 (lambda m: m["layers"][0].update(urls=["http://127.0.0.1/private"]), "LAYER_MEDIA_OR_FOREIGN_URL"),
                 (lambda m: m.update(layers=[]), "LAYERS_SHAPE"))
        for change, expected in cases:
            with self.subTest(expected=expected), self.assertRaisesRegex(registry.ReadbackError, "^" + expected + "$"):
                self.verify(image_fixture(manifest_change=change))
        image, first, second = image_fixture()
        with self.assertRaisesRegex(registry.ReadbackError, "^MEDIA_TYPE$"):
            self.verify((image, registry.Response(200, {"content-type": "text/html"}, first.body), second))

    def test_bounded_json_rejects_duplicate_keys_nonfinite_invalid_and_oversize(self):
        for data, expected in ((b'{"a":1,"a":2}', "JSON_DUPLICATE_KEY"),
                               (b'{"a":NaN}', "JSON_NONFINITE"), (b"not json", "JSON_INVALID"),
                               (b"[]", "JSON_OBJECT_REQUIRED"), (b"x" * (registry.MAX_BYTES + 1), "RESPONSE_SIZE")):
            with self.subTest(expected=expected), self.assertRaisesRegex(registry.ReadbackError, "^" + expected + "$"):
                registry.bounded_json(data)

    def challenge(self, **changes):
        fields = {"realm": "https://ghcr.io/token", "service": "ghcr.io",
                  "scope": "repository:loveoverflowcom/cantos-server:pull"} | changes
        return "Bearer " + ",".join(k + '=\"' + v + '\"' for k, v in fields.items())

    def test_private_token_scope_and_cdn_redirect_never_forward_credentials(self):
        image, first, config = image_fixture()
        transport = RecordingTransport([
            registry.Response(401, {"www-authenticate": self.challenge().replace("Bearer", "bearer").replace("realm=", "Realm=")}, b""),
            registry.Response(200, {"content-type": "application/json"}, b'{"token":"fixture-token"}'), first,
            registry.Response(307, {"location": registry.CDN_ORIGIN + "/fixture?signature=private-fixture"}, b""), config])
        client = registry.GhcrClient("cantos-server", transport, "Basic fixture-basic")
        client.get("manifests", image["ref"].split("@")[1])
        client.get("blobs", image["config_digest"])
        self.assertNotIn("Authorization", transport.calls[0][1])
        self.assertTrue(transport.calls[1][0].startswith("https://ghcr.io/token?"))
        self.assertIn("%3Apull", transport.calls[1][0])
        self.assertNotIn("push", transport.calls[1][0])
        self.assertEqual(transport.calls[1][1]["Authorization"], "Basic fixture-basic")
        self.assertEqual(transport.calls[2][1]["Authorization"], "Bearer fixture-token")
        self.assertEqual(transport.calls[3][1]["Authorization"], "Bearer fixture-token")
        self.assertNotIn("Authorization", transport.calls[4][1])

    def test_foreign_realms_wrong_scopes_and_duplicate_challenges_never_receive_credentials(self):
        for challenge in (self.challenge(realm="https://evil.invalid/token"),
                          self.challenge(realm="http://ghcr.io/token"),
                          self.challenge(scope="repository:loveoverflowcom/cantos-server:pull,push"),
                          self.challenge(scope="repository:loveoverflowcom/cantos-identity:pull"),
                          self.challenge() + ',realm="https://ghcr.io/token"',
                          self.challenge() + ',Realm="https://ghcr.io/token"'):
            transport = RecordingTransport([registry.Response(401, {"www-authenticate": challenge}, b"")])
            with self.assertRaises(registry.ReadbackError):
                registry.GhcrClient("cantos-server", transport, "Basic never-send").get("manifests", "sha256:" + "a" * 64)
            self.assertEqual(len(transport.calls), 1)
            self.assertNotIn("Authorization", transport.calls[0][1])

    def test_pair_tokens_are_repository_scoped_and_never_appear_in_observation(self):
        with tempfile.TemporaryDirectory() as temporary:
            source, args, _, responses = bind(Path(temporary))
            authfile = Path(temporary) / "auth.json"
            encoded = base64.b64encode(b"fixture-user:fixture-private-token").decode()
            authfile.write_bytes(raw({"auths": {"ghcr.io": {"auth": encoded}}}))
            authfile.chmod(0o600)
            args.authfile = authfile
            sequence = []
            for i, service in enumerate(oci.SERVICES):
                challenge = self.challenge(scope="repository:loveoverflowcom/" + service + ":pull")
                sequence.extend((registry.Response(401, {"www-authenticate": challenge}, b""),
                                 registry.Response(200, {"content-type": "application/json"},
                                                   raw({"token": "fixture-token-" + str(i)})),
                                 responses[i * 2], responses[i * 2 + 1]))
            transport = RecordingTransport(sequence)
            with patch.object(registry, "HttpsTransport", return_value=transport):
                observation = oci.execute(source, args)
            self.assertEqual(len(transport.calls), 8)
            self.assertNotIn("Authorization", transport.calls[4][1])
            self.assertEqual(transport.calls[2][1]["Authorization"], "Bearer fixture-token-0")
            self.assertEqual(transport.calls[6][1]["Authorization"], "Bearer fixture-token-1")
            self.assertNotIn("fixture-", json.dumps(observation))
            self.assertNotIn(encoded, json.dumps(observation))

    def test_redirect_origins_and_token_endpoint_redirects_refuse_without_external_request(self):
        targets = ("http://127.0.0.1/private", "https://evil.invalid/blob", "https://ghcr.io/token",
                   "https://user:password@pkg-containers.githubusercontent.com/blob",
                   "https://pkg-containers.githubusercontent.com.evil.invalid/blob", "https://[invalid",
                   registry.CDN_ORIGIN + "/unsafe path?private=secret")
        for target in targets:
            transport = RecordingTransport([registry.Response(307, {"location": target}, b"")])
            with self.subTest(target=target), self.assertRaisesRegex(registry.ReadbackError, "^REDIRECT_FORBIDDEN$"):
                registry.GhcrClient("cantos-server", transport).get("blobs", "sha256:" + "a" * 64)
            self.assertEqual(len(transport.calls), 1)
        transport = RecordingTransport([registry.Response(401, {"www-authenticate": self.challenge()}, b""),
                                        registry.Response(302, {"location": "https://evil.invalid"}, b"")])
        with self.assertRaisesRegex(registry.ReadbackError, "^HTTP_302$"):
            registry.GhcrClient("cantos-server", transport, "Basic secret").get("manifests", "sha256:" + "a" * 64)
        self.assertEqual(len(transport.calls), 2)

    def test_http_auth_not_found_errors_are_precise_and_never_include_remote_bodies(self):
        for status in (403, 404, 429, 500):
            transport = RecordingTransport([registry.Response(status, {}, b"private-remote-detail")])
            with self.assertRaisesRegex(registry.ReadbackError, "^HTTP_" + str(status) + "$"):
                registry.GhcrClient("cantos-server", transport).get("manifests", "sha256:" + "a" * 64)
            self.assertEqual(len(transport.calls), 1)
        transport = RecordingTransport([registry.Response(401, {"www-authenticate": self.challenge()}, b""),
                                        registry.Response(401, {}, b"private-token-detail")])
        with self.assertRaisesRegex(registry.ReadbackError, "^HTTP_401$"):
            registry.GhcrClient("cantos-server", transport).get("manifests", "sha256:" + "a" * 64)
        self.assertEqual(len(transport.calls), 2)

    def test_authfile_is_explicit_private_owned_and_has_no_credential_helper_execution(self):
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary) / "auth.json"
            auth = base64.b64encode(b"fixture-user:fixture-private-token").decode()
            path.write_bytes(raw({"auths": {"ghcr.io": {"auth": auth}}}))
            path.chmod(0o600)
            self.assertEqual(registry.load_authfile(path), "Basic " + auth)
            path.chmod(0o644)
            with self.assertRaisesRegex(registry.ReadbackError, "^AUTHFILE_PERMISSIONS$"):
                registry.load_authfile(path)
            path.chmod(0o600)
            path.write_bytes(raw({"auths": {"ghcr.io": {"auth": auth}}, "credsStore": "never-execute"}))
            with self.assertRaisesRegex(registry.ReadbackError, "^AUTHFILE_HELPER_UNSUPPORTED$"):
                registry.load_authfile(path)
            link = path.parent / "link.json"
            link.symlink_to(path)
            with self.assertRaisesRegex(registry.ReadbackError, "^AUTHFILE_INVALID$"):
                registry.load_authfile(link)

    def test_https_transport_has_no_automatic_redirects_and_safe_network_error_codes(self):
        transport = registry.HttpsTransport()
        self.assertIsNone(registry.NoRedirect().redirect_request(None, None, 307, "", {}, "https://evil.invalid"))
        for reason, expected in ((ssl.SSLCertVerificationError("private-detail"), "TLS_VERIFICATION"),
                                 (socket.gaierror("private-detail"), "NETWORK_DNS"),
                                 (TimeoutError("private-detail"), "NETWORK_TIMEOUT"),
                                 (ConnectionError("private-detail"), "NETWORK_UNAVAILABLE")):
            with patch.object(transport.opener, "open", side_effect=urllib.error.URLError(reason)):
                with self.assertRaisesRegex(registry.ReadbackError, "^" + expected + "$"):
                    transport("https://ghcr.io/v2/", {}, registry.MAX_BYTES)

    def test_real_http_response_reader_bounds_body_headers_and_protocol_failures(self):
        class WireSocket:
            def __init__(self, wire):
                self.stream = io.BytesIO(wire)

            def makefile(self, *args):
                return self.stream

        cases = ((b"Content-Length: " + str(registry.MAX_BYTES + 1).encode() + b"\r\n", b"", "RESPONSE_SIZE"),
                 (b"", b"x" * (registry.MAX_BYTES + 1), "RESPONSE_SIZE"),
                 (b"Content-Length: 10\r\n", b"{}", "NETWORK_TRUNCATED"),
                 (b"Content-Type: application/json\r\nContent-Type: text/html\r\n", b"{}", "HTTP_AMBIGUOUS_HEADERS"))
        transport = registry.HttpsTransport()
        for headers, body, expected in cases:
            response = http.client.HTTPResponse(WireSocket(b"HTTP/1.1 200 OK\r\n" + headers + b"\r\n" + body))
            response.begin()
            with patch.object(transport.opener, "open", return_value=response):
                with self.subTest(expected=expected), self.assertRaisesRegex(registry.ReadbackError, "^" + expected + "$"):
                    transport("https://ghcr.io/v2/", {}, registry.MAX_BYTES)
        with patch.object(transport.opener, "open", side_effect=http.client.InvalidURL("private-signed-url")):
            with self.assertRaisesRegex(registry.ReadbackError, "^NETWORK_PROTOCOL$"):
                transport("https://ghcr.io/v2/", {}, registry.MAX_BYTES)


if __name__ == "__main__":
    unittest.main()
