"""Linux-only exact-image qualification primitive owned by `xtask release oci-qualify`.

Rootful Podman runs in a private, run-owned image store. No host service, systemd unit,
production path, admission, registry write or deployment is touched. Failure never becomes PASS.
"""

import argparse
import copy
import gzip
import hashlib
import io
import json
import os
from pathlib import Path, PurePosixPath
import platform
import re
import select
import shutil
import socket
import stat
import subprocess
import sys
import tarfile
import tempfile
import time
import uuid

import release_artifact as contract
import release_oci as oci
import release_oci_build as build
import release_oci_registry as registry

UIDS = {"cantos-server": 10001, "cantos-identity": 10002}
FIXTURE_FILES = ("scripts/release_oci_qualify.py", "scripts/release_oci_fixture.py")
SERVER_ENV = {"CANTOS_ENV": "production", "CANTOS_BIND_ADDRESS": "127.0.0.1:8080",
              "CANTOS_SERVER_DATABASE": "/database/server.db",
              "CANTOS_STUDIO_WEB_ORIGIN": "https://studio.example.test",
              "CANTOS_THEATRE_WEB_ORIGIN": "https://theatre.example.test",
              "CANTOS_IDENTITY_SOCKET": "/identity/identity.sock", "CANTOS_IDENTITY_PEER_UID": "10002",
              "CANTOS_IDENTITY_PEER_POLICY": "direct_peer"}
IDENTITY_ENV = {"CANTOS_IDENTITY_DATABASE": "/database/identity.db",
                "CANTOS_IDENTITY_SOCKET": "/identity/identity.sock", "CANTOS_IDENTITY_SOCKET_MODE": "0660",
                "CANTOS_IDENTITY_SERVER_UIDS": "10001",
                "CANTOS_IDENTITY_SIGNUP_LINK_BASE": "https://theatre.example.test/signup",
                "CANTOS_IDENTITY_MAIL_PROVIDER": "smtp", "CANTOS_IDENTITY_SMTP_HOST": "127.0.0.1",
                "CANTOS_IDENTITY_SMTP_PORT": "2525", "CANTOS_IDENTITY_SMTP_TLS": "loopback-plaintext",
                "CANTOS_IDENTITY_SMTP_FROM": "cantos@example.test"}
RUNTIME_POLICY = {"network": "none; shared pair namespace; loopback SMTP/HTTP only", "read_only": True,
                  "capabilities": [], "no_new_privileges": True, "uids": UIDS,
                  "database_mode": "0700", "socket_directory": "10002:10001:2750",
                  "socket_mode": "0660", "server_socket_mount": "read-only"}


def require(condition, code):
    contract.require(condition, code)


def sha(data):
    return hashlib.sha256(data).hexdigest()


def fixture_configuration(root):
    return {"server": SERVER_ENV, "identity": IDENTITY_ENV, "runtime": RUNTIME_POLICY,
            "files": {name: sha((root / name).read_bytes()) for name in FIXTURE_FILES}}


def validate_input(root, args):
    manifest = contract.read_json(args.manifest, contract.digest(args.manifest_digest))
    plan = oci.candidate_plan(manifest, manifest_digest=args.manifest_digest, source_sha=args.source_sha,
                              configuration_digest=args.configuration_digest, platform=args.platform)
    require(oci.git(root, "rev-parse", "HEAD").decode().strip() == args.source_sha, "QUALIFIER_SOURCE_MISMATCH")
    require(not oci.git(root, "status", "--porcelain", "--untracked-files=normal").strip(), "QUALIFIER_SOURCE_DIRTY")
    require(build.configuration_identity(root, args.source_sha, args.platform) == args.configuration_digest,
            "BUILD_CONFIGURATION_MISMATCH")
    require(build.compatibility(root, args.source_sha) == manifest["compatibility"], "COMPATIBILITY_MISMATCH")
    oci.verify_bundle(root, args.manifest.parent / "runtime.tar", manifest)
    path = args.manifest.parent / "evidence.json"
    require(contract.file_record(path, path.parent) == manifest["evidence"], "INPUT_EVIDENCE_DIGEST_MISMATCH")
    evidence = contract.read_json(path, manifest["evidence"]["sha256"])
    oci.fields(evidence, "schema_version source_sha configuration_sha256 platform images producer gates", "INPUT_EVIDENCE_FIELDS")
    require(all(evidence.get(key) == manifest[key] for key in
                ("schema_version", "source_sha", "configuration_sha256", "platform", "images", "producer")),
            "INPUT_EVIDENCE_IDENTITY_MISMATCH")
    oci.fields(evidence["gates"], " ".join(oci.GATES), "INPUT_EVIDENCE_GATES")
    for gate, item in evidence["gates"].items():
        oci.fields(item, "status record_sha256", "INPUT_GATE_FIELDS")
        require(item["status"] in ("PASS", "FAIL", "NOT_RUN"), "INPUT_GATE_STATUS")
        record = contract.read_json(path.parent / "gate-records" / (gate + ".json"), item["record_sha256"])
        require(record.get("gate") == gate and record.get("status") == item["status"] and
                record.get("source_sha") == manifest["source_sha"], "INPUT_GATE_RECORD_MISMATCH")
    # Input gate declarations are never trusted or promoted. Every result comes from this run.
    return manifest, plan


def native_host(which=platform, uid=os.geteuid()):
    require(which.system() == "Linux" and build.MACHINES.get(which.machine()) in oci.PLATFORMS,
            "LINUX_NATIVE_REQUIRED")
    require(uid == 0, "ROOTFUL_DISPOSABLE_LINUX_REQUIRED")
    return build.MACHINES[which.machine()]


def inspect_identity(service, image, manifest, inspection):
    digest = build.check_local_image(service, inspection, manifest["source_sha"],
                                     manifest["configuration_sha256"], manifest["platform"])
    require(digest == image["config_digest"], "PULLED_CONFIG_DIGEST_MISMATCH")
    require(inspection["Config"]["User"] == f"{UIDS[service]}:{UIDS[service]}", "EXACT_SERVICE_UID_REQUIRED")
    require(image["ref"] in inspection.get("RepoDigests", []), "PULLED_REGISTRY_DIGEST_MISMATCH")


def check_runtime_environment(service, entries, identity_server_uid=10001):
    observed = {}
    for entry in entries:
        key, separator, value = entry.partition("=")
        require(separator and key not in observed, "RUNNING_ENVIRONMENT_AMBIGUOUS")
        observed[key] = value
    expected = SERVER_ENV if service == "cantos-server" else {
        **IDENTITY_ENV, "CANTOS_IDENTITY_SERVER_UIDS": str(identity_server_uid)}
    require(all(observed.get(key) == value for key, value in expected.items()), "RUNNING_ENVIRONMENT_MISMATCH")


def peer_refusal_events(data):
    """Return only whitelisted static events, never raw logs or request values."""
    require(len(data) <= 65536, "IDENTITY_EVENT_LIMIT")
    found = set()
    for line in data.splitlines():
        if not line.startswith(b"{"):
            continue
        value = registry.bounded_json(line)
        if value == {"event": "ready", "mail": "configured"}:
            found.add("ready")
        if value == {"event": "connection_refused", "detail": "peer is not an allowed caller"}:
            found.add("connection_refused")
    require(found == {"ready", "connection_refused"}, "IDENTITY_PEER_REFUSAL_NOT_OBSERVED")
    return ["identity-ready-event", "identity-connection-refused-event"]


def scan_layer(stream):
    """Bounded content policy over *every* layer, including files deleted by a later layer.

    This is a disclosed allow/deny policy, not a universal secret-detection claim.
    """
    binaries = set()
    with tarfile.open(fileobj=stream, mode="r|") as layer:
        count = total = 0
        for member in layer:
            count += 1
            total += member.size
            require(count <= 200000 and total <= 2 * 1024**3, "LAYER_CONTENT_LIMIT")
            path = PurePosixPath(member.name.removeprefix("./"))
            require(not path.is_absolute() and ".." not in path.parts, "LAYER_UNSAFE_PATH")
            name = path.as_posix()
            require(not any(part in (".aws", ".ssh", ".gnupg", ".env", "node_modules") for part in path.parts)
                    and path.suffix not in (".db", ".sqlite", ".sqlite3", ".key", ".mp3", ".opus")
                    and path.name not in ("auth.json", "credentials.json", "service-account.json"),
                    "LAYER_FORBIDDEN_ASSET")
            if name.startswith("usr/local/bin/") and member.isfile():
                binaries.add(path.name)
            if member.isfile():
                with layer.extractfile(member) as source:
                    previous = b""
                    while block := source.read(1024 * 1024):
                        data = previous + block
                        require(not re.search(rb"-----BEGIN (?:RSA |OPENSSH |EC )?PRIVATE KEY-----\r?\n[A-Za-z0-9+/]{32}", data)
                                and b"CANTOS_QUALIFICATION_HOST_CANARY" not in data,
                                "LAYER_CREDENTIAL_MARKER")
                        previous = data[-128:]
    return binaries


def check_archive(path, service, image):
    """Verify saved config, compressed descriptors and uncompressed diff_ids without extraction."""
    binaries = set()
    with tarfile.open(path, "r:") as archive:
        members = archive.getmembers()
        require(len(members) <= 1024 and len({m.name for m in members}) == len(members), "OCI_ARCHIVE_MEMBERS")
        entries = {m.name: m for m in members}

        def blob(descriptor, limit):
            digest = oci.oci_digest(descriptor["digest"])
            name = "blobs/sha256/" + digest[7:]
            member = entries[name]
            require(member.isfile() and member.size == descriptor["size"] and member.size <= limit,
                    "OCI_BLOB_SIZE")
            with archive.extractfile(member) as source:
                data = source.read(limit + 1)
            require("sha256:" + sha(data) == digest, "OCI_BLOB_DIGEST")
            return data

        index = registry.bounded_json(archive.extractfile("index.json").read(registry.MAX_BYTES + 1))
        require(len(index["manifests"]) == 1, "OCI_ARCHIVE_SINGLE_MANIFEST")
        document = registry.bounded_json(blob(index["manifests"][0], registry.MAX_BYTES))
        require(document["config"]["digest"] == image["config_digest"], "SAVED_CONFIG_DIGEST_MISMATCH")
        config = registry.bounded_json(blob(document["config"], registry.MAX_BYTES))
        require(len(document["layers"]) == len(config["rootfs"]["diff_ids"]), "LAYER_COUNT_MISMATCH")
        for descriptor, diff_id in zip(document["layers"], config["rootfs"]["diff_ids"], strict=True):
            raw = blob(descriptor, 512 * 1024**2)
            media = descriptor["mediaType"]
            require(media in ("application/vnd.oci.image.layer.v1.tar", "application/vnd.oci.image.layer.v1.tar+gzip"),
                    "LAYER_COMPRESSION_UNSUPPORTED")
            stream = gzip.GzipFile(fileobj=io.BytesIO(raw)) if media.endswith("+gzip") else io.BytesIO(raw)
            # Bound decompression and hash before scanning the same bytes.
            decoded = stream.read(512 * 1024**2 + 1)
            require(len(decoded) <= 512 * 1024**2 and "sha256:" + sha(decoded) == diff_id, "LAYER_DIFF_ID_MISMATCH")
            binaries |= scan_layer(io.BytesIO(decoded))
    expected = {service, "cantos-migrate"} if service == "cantos-server" else {service}
    require(binaries == expected, "IMAGE_BINARY_SET")
    return {"layers": len(document["layers"]), "policy": "all-layer-paths-and-private-key-markers-v1",
            "config_digest": image["config_digest"], "diff_ids": config["rootfs"]["diff_ids"]}


class Fixture:
    def __init__(self, root, work, manifest, authfile):
        self.root, self.work, self.manifest, self.authfile = root, work, manifest, authfile
        self.names = []
        self.driver = None
        self.operation_context = "setup"
        self.prefix = "cantos-qualify-" + uuid.uuid4().hex
        self.podman = ["podman", "--root", str(work / "store"), "--runroot", str(work / "run"),
                       "--storage-driver", "vfs"]
        self.env = {"PATH": os.environ.get("PATH", "/usr/bin:/bin"), "HOME": str(work / "home"),
                    "LANG": "C.UTF-8", "TMPDIR": str(work)}
        (work / "home").mkdir(mode=0o700)
        self.socket_dir = work / "socket"
        self.socket_dir.mkdir()
        os.chown(self.socket_dir, 10002, 10001)
        self.socket_dir.chmod(0o2750)
        self.databases = self.new_databases("initial")

    def sh(self, args, expected=0, timeout=120, *, include_stderr=False):
        result = subprocess.run(args, cwd=self.root, env=self.env, stdin=subprocess.DEVNULL,
                                stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=timeout, check=False)
        require(result.returncode == expected, "COMMAND_" + Path(args[0]).name + "_EXIT_" + str(result.returncode))
        output = result.stdout + (b"\n" + result.stderr if include_stderr else b"")
        require(len(output) <= 4 * 1024**2, "COMMAND_OUTPUT_LIMIT")
        return output

    def pod(self, *args, expected=0, timeout=120, include_stderr=False):
        try:
            return self.sh([*self.podman, *map(str, args)], expected, timeout, include_stderr=include_stderr)
        except ValueError as error:
            # Identify the operation without logging arguments, auth paths or remote stderr.
            prefix = "COMMAND_podman_EXIT_"
            if str(error).startswith(prefix):
                action = str(args[0]).strip("-").upper()
                require(re.fullmatch(r"[A-Z-]{1,30}", action), "PODMAN_OPERATION")
                context = getattr(self, "operation_context", None)
                label = (context + ":") if context else ""
                raise ValueError(label + "PODMAN_" + action + "_EXIT_" + str(error)[len(prefix):]) from None
            raise

    def new_databases(self, name):
        result = {}
        for service in oci.SERVICES:
            path = self.work / name / service
            path.mkdir(parents=True, mode=0o700)
            os.chown(path, UIDS[service], UIDS[service])
            result[service] = path
        return result

    def image(self, service):
        return self.manifest["images"][service]["ref"]

    def options(self, service, *, database=None, socket_mount=True):
        mount = database or self.databases[service]
        result = ["--pull=never", "--read-only", "--cap-drop=ALL", "--security-opt=no-new-privileges",
                  "--tmpfs", "/tmp:rw,noexec,nosuid,size=64m", "--mount", f"type=bind,src={mount},dst=/database"]
        if socket_mount:
            readonly = ",ro=true" if service == "cantos-server" else ""
            result += ["--mount", f"type=bind,src={self.socket_dir},dst=/identity{readonly}"]
        for key, value in (SERVER_ENV if service == "cantos-server" else IDENTITY_ENV).items():
            result += ["--env", key + "=" + value]
        return result

    def migrate(self):
        for service in oci.SERVICES:
            self.operation_context = "migrate-" + service
            entry = "/usr/local/bin/cantos-migrate" if service == "cantos-server" else "/usr/local/bin/cantos-identity"
            args = [] if service == "cantos-server" else ["migrate"]
            self.pod("run", "--rm", "--network=none", *self.options(service), "--entrypoint", entry,
                     self.image(service), *args)

    def start(self, *, identity_server_uid=10001):
        require(identity_server_uid in (10001, 10003), "FIXTURE_CALLER_UID")
        identity = self.prefix + "-identity"
        server = self.prefix + "-server"
        network = "none" if self.driver is None else f"ns:/proc/{self.driver.pid}/ns/net"
        for service, name, network in (("cantos-identity", identity, network),
                                       ("cantos-server", server, "container:" + identity)):
            self.operation_context = "start-" + service + "-allowed-" + str(identity_server_uid)
            self.names.append(name)  # Also clean a container whose create/start outcome is uncertain.
            override = ["--env", "CANTOS_IDENTITY_SERVER_UIDS=" + str(identity_server_uid)] if service == "cantos-identity" else []
            self.pod("run", "-d", "--name", name, "--network", network, *self.options(service), *override,
                     self.image(service), *(["serve"] if service == "cantos-identity" else []))
        self.identity, self.server = identity, server
        namespaces = []
        for service, name in (("cantos-identity", identity), ("cantos-server", server)):
            observed = json.loads(self.pod("inspect", name))[0]
            require(observed["State"]["Running"] and
                    "sha256:" + contract.digest(observed["Image"].removeprefix("sha256:")) ==
                    self.manifest["images"][service]["config_digest"],
                    "RUNNING_EXACT_IMAGE_REQUIRED")
            require(observed["Config"]["User"] == f"{UIDS[service]}:{UIDS[service]}", "RUNNING_UID_MISMATCH")
            check_runtime_environment(service, observed["Config"]["Env"], identity_server_uid)
            process = Path(f"/proc/{observed['State']['Pid']}/status").read_text()
            namespaces.append(os.readlink(f"/proc/{observed['State']['Pid']}/ns/net"))
            facts = dict(line.split(":", 1) for line in process.splitlines() if ":" in line)
            require(set(facts["Uid"].split()) == {str(UIDS[service])} and
                    set(facts["Gid"].split()) == {str(UIDS[service])} and
                    int(facts["CapEff"].strip(), 16) == 0 and facts["NoNewPrivs"].strip() == "1",
                    "PROCESS_UID_CAPABILITIES_OR_PRIVILEGES")
            mounts = {m["Destination"]: m for m in observed["Mounts"] if m["Type"] == "bind"}
            require(set(mounts) == {"/database", "/identity"} and
                    mounts["/database"]["Source"] == str(self.databases[service]) and mounts["/database"]["RW"] and
                    mounts["/identity"]["Source"] == str(self.socket_dir) and
                    mounts["/identity"]["RW"] == (service == "cantos-identity"), "RUNNING_MOUNT_MISMATCH")
        self.pid = json.loads(self.pod("inspect", identity))[0]["State"]["Pid"]
        require(len(set(namespaces)) == 1 and namespaces[0] != os.readlink("/proc/self/ns/net"),
                "SHARED_PRIVATE_NETWORK_NAMESPACE_REQUIRED")
        if self.driver is None:
            self.sh(["nsenter", "-t", str(self.pid), "-n", "ip", "link", "set", "lo", "up"])
            self.driver = subprocess.Popen(["nsenter", "-t", str(self.pid), "-n", sys.executable,
                                            str(self.root / "scripts/release_oci_fixture.py")], cwd=self.root,
                                           env=self.env, stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                                           stderr=subprocess.DEVNULL, text=True)
            require(self.read_driver() == {"ready": True}, "DRIVER_READY")
        # Later starts join the *driver's* retained namespace, see restart().
        deadline = time.monotonic() + 30
        while True:
            result = self.call("health", must_pass=False)
            if result["status"] == "PASS":
                break
            require(time.monotonic() < deadline, "PRODUCTION_HTTP_NOT_READY")
            time.sleep(0.2)
        self.wait_socket()

    def read_driver(self):
        require(select.select([self.driver.stdout], [], [], 60)[0], "DRIVER_TIMEOUT")
        value = self.driver.stdout.readline(16385)
        require(value and len(value) < 16384, "DRIVER_OUTPUT_LIMIT")
        return registry.bounded_json(value.encode())

    def call(self, action, must_pass=True, **kwargs):
        self.driver.stdin.write(json.dumps({"action": action, **kwargs}) + "\n")
        self.driver.stdin.flush()
        result = self.read_driver()
        require(not must_pass or result["status"] == "PASS", result.get("check", "DRIVER_FAILURE"))
        return result

    def stop(self):
        for name in (self.server, self.identity):
            self.operation_context = "stop-server" if name == self.server else "stop-identity"
            if json.loads(self.pod("inspect", name))[0]["State"]["Running"]:
                self.pod("stop", "--time", "15", name)
            require(not json.loads(self.pod("inspect", name))[0]["State"]["Running"], "COLD_BACKUP_REQUIRES_STOP")
            self.pod("rm", name)
            self.names.remove(name)

    def restart(self):
        # The driver retains the namespace while both production owners are stopped.
        self.start()

    def isolation(self):
        # Earned credentials must work before the wrong-peer control. An unknown account
        # would return 401 even if the UID boundary were broken, making that control vacuous.
        checks = self.call("persisted")["checks"]
        with socket.socket() as sentinel:
            sentinel.bind(("127.0.0.1", 0))
            sentinel.listen()
            with socket.create_connection(sentinel.getsockname(), timeout=2):
                pass
            checks += self.call("isolation", host_port=sentinel.getsockname()[1])["checks"]
        metadata = self.socket_dir.joinpath("identity.sock").stat()
        require(stat.S_ISSOCK(metadata.st_mode) and metadata.st_uid == 10002 and metadata.st_gid == 10001
                and stat.S_IMODE(metadata.st_mode) == 0o660, "SOCKET_UID_GID_MODE")
        for service in oci.SERVICES:
            own = "server.db" if service == "cantos-server" else "identity.db"
            other = next(s for s in oci.SERVICES if s != service)
            foreign = "server.db" if other == "cantos-server" else "identity.db"
            self.operation_context = "isolation-own-database-" + service
            self.pod("run", "--rm", "--network=none", *self.options(service), "--entrypoint", "/bin/sh",
                     self.image(service), "-c", "test -r /database/" + own)
            self.operation_context = "isolation-foreign-database-" + service
            self.pod("run", "--rm", "--network=none", *self.options(service, database=self.databases[other]),
                     "--entrypoint", "/bin/sh", self.image(service), "-c", "test -r /database/" + foreign, expected=1)
            checks += [service + "-own-db-positive", service + "-foreign-db-denied"]
        canary = self.work / "host-credential"
        canary.write_text("CANTOS_QUALIFICATION_HOST_CANARY\n")
        canary.chmod(0o644)
        executable = self.work / "host-executable"
        executable.write_text("#!/bin/sh\nexit 42\n")
        executable.chmod(0o755)
        for service in oci.SERVICES:
            for path, command, expected in ((canary, "cat /host-canary", 0), (executable, "/host-canary", 42)):
                self.operation_context = "isolation-positive-" + path.name + "-" + service
                self.pod("run", "--rm", "--network=none", *self.options(service), "--mount",
                         f"type=bind,src={path},dst=/host-canary,ro=true", "--entrypoint", "/bin/sh",
                         self.image(service), "-c", command, expected=expected)
            self.operation_context = "isolation-negative-credential-" + service
            self.pod("run", "--rm", "--network=none", *self.options(service), "--entrypoint", "/bin/sh",
                     self.image(service), "-c", "cat /host-canary", expected=1)
            self.operation_context = "isolation-negative-executable-" + service
            self.pod("run", "--rm", "--network=none", *self.options(service), "--entrypoint", "/bin/sh",
                     self.image(service), "-c", "/host-canary", expected=127)
            checks += [service + "-credential-mount-positive-and-absent-negative",
                       service + "-executable-mount-positive-and-absent-negative"]
        # A real Server UID rejected by Identity's OS-peer allowlist: configure the wrong caller.
        self.operation_context = "isolation-stop-identity"
        self.pod("stop", "--time", "15", self.identity)
        checks += self.call("unavailable")["checks"]
        # Server depends on Identity's network namespace. Remove the owned pair in
        # dependency order, then recreate both in the namespace retained by the driver.
        self.stop()
        self.start(identity_server_uid=10003)
        checks += self.call("denied_peer")["checks"]
        checks += peer_refusal_events(self.pod("logs", self.identity, include_stderr=True))
        self.stop()
        self.restart()
        checks += self.call("persisted")["checks"]
        return checks + ["uds-mode-and-peer-uid-negative"]

    def wait_socket(self):
        for _ in range(100):
            if (self.socket_dir / "identity.sock").exists():
                return
            time.sleep(0.1)
        raise ValueError("IDENTITY_SOCKET_NOT_READY")

    def close(self):
        for name in reversed(self.names):
            # Only this invocation's recorded names; never prune another task's resources.
            subprocess.run([*self.podman, "rm", "--force", name], env=self.env, stdout=subprocess.DEVNULL,
                           stderr=subprocess.DEVNULL, timeout=30, check=False)
        if self.driver is not None:
            self.driver.stdin.close()
            try:
                self.driver.wait(timeout=5)
            except subprocess.TimeoutExpired:
                self.driver.terminate()
                self.driver.wait(timeout=5)


def directory_record(path, *, allow_empty=False):
    records = {}
    for entry in sorted(path.rglob("*")):
        info = entry.lstat()
        require(stat.S_ISREG(info.st_mode) and info.st_nlink == 1, "BACKUP_REGULAR_FILES_ONLY")
        records[str(entry.relative_to(path))] = {"sha256": sha(entry.read_bytes()), "bytes": info.st_size,
                                               "uid": info.st_uid, "gid": info.st_gid,
                                               "mode": stat.S_IMODE(info.st_mode)}
    require((records or allow_empty) and len(records) <= 4096, "BACKUP_FILE_COUNT")
    return records


def check_unmigrated_layout(service, files):
    """Serve may acquire the owner's empty lock, but must never initialize database bytes.

    OwnedDatabase::open deliberately takes OwnerLock before observing/refusing an absent DB.
    The lock is not schema or persistence initialization; retain its exact private layout.
    """
    database = service.removeprefix("cantos-") + ".db"
    owner = database + ".owner"
    require(set(files) <= {owner}, "SERVE_CREATED_DATABASE")
    if owner in files:
        record = files[owner]
        require(record["bytes"] == 0 and record["uid"] == UIDS[service] and record["gid"] == UIDS[service]
                and record["mode"] == 0o600, "SERVE_OWNER_LOCK_LAYOUT")


def cold_copy(source, destination, expected=None):
    before = directory_record(source)
    require(expected is None or before == expected, "PINNED_BACKUP_CHANGED")
    shutil.copytree(source, destination, copy_function=shutil.copy2)
    source_info = source.stat()
    os.chown(destination, source_info.st_uid, source_info.st_gid)
    for name, record in before.items():
        os.chown(destination / name, record["uid"], record["gid"])
    require(directory_record(source) == before == directory_record(destination), "COLD_COPY_HASH_OR_METADATA_MISMATCH")
    return before


def run_gates(fixture, manifest, plan):
    f = fixture
    registry.observe(manifest, plan, authfile=f.authfile)
    version = f.pod("--version").decode().strip()
    require(re.fullmatch(r"podman version [A-Za-z0-9.+-]{1,40}", version), "PODMAN_VERSION_FORMAT")
    layers = {}
    for service in oci.SERVICES:
        auth = ["--authfile", str(f.authfile)] if f.authfile else []
        f.pod("pull", *auth, f.image(service), timeout=600)
        inspect_identity(service, manifest["images"][service], manifest, json.loads(f.pod("image", "inspect", f.image(service)))[0])
        archive = f.work / (service + ".tar")
        f.pod("save", "--format", "oci-archive", "--output", archive, f.image(service), timeout=300)
        layers[service] = check_archive(archive, service, manifest["images"][service])
        archive.unlink()  # Only the exact owned export; layer hashes remain in the receipt.
    # Fail before migrate: neither production process may initialize a missing database.
    for service in oci.SERVICES:
        f.pod("run", "--rm", "--network=none", *f.options(service), f.image(service),
              *(["serve"] if service == "cantos-identity" else []), expected=1)
        check_unmigrated_layout(service, directory_record(f.databases[service], allow_empty=True))
    f.migrate()
    f.start()
    yield "image-pair", {"checks": ["registry-manifest-config", "exact-running-configs-uids-mounts",
                                    "all-layer-policy", "serve-before-migrate-refused", "production-http-readiness"],
                         "layers": layers, "runtime": {"podman": version, "kernel": platform.release(),
                                                        "storage_driver": "vfs", "principal": "rootful-disposable"}}
    yield "native-e2e", {"checks": f.call("signup")["checks"]}
    yield "native-isolation", {"checks": f.isolation()}
    f.stop()
    backups = {service: cold_copy(path, f.work / "backup" / service) for service, path in f.databases.items()}
    # Idempotent migration on a real populated cold copy, then actual HTTP/auth from that copy.
    f.databases = {service: f.work / "migrated" / service for service in oci.SERVICES}
    for service in oci.SERVICES:
        cold_copy(f.work / "backup" / service, f.databases[service], backups[service])
    f.migrate()
    f.restart()
    checks = f.call("persisted")["checks"]
    checks += f.call("revoke")["checks"]
    f.stop()
    f.restart()
    checks += f.call("revoked")["checks"]
    f.stop()
    # Restore into fresh roots, leaving the migrated roots and cold backup intact.
    f.databases = {service: f.work / "restored" / service for service in oci.SERVICES}
    for service in oci.SERVICES:
        cold_copy(f.work / "backup" / service, f.databases[service], backups[service])
    f.migrate()
    f.restart()
    checks += f.call("persisted")["checks"]
    yield "migration-restore", {"checks": checks + ["cold-complete-directory-copy", "populated-idempotent-migrate",
                                                  "restore-to-fresh-roots", "restored-http-auth-session"],
                                "backup_files": backups,
                                "scope": "fresh current-schema migration and same-schema restore; no historical schema upgrade"}


def write_result(output, manifest, input_path, input_digest, records, configuration, failure):
    if not output.exists():
        output.mkdir(parents=True, mode=0o700)
        (output / "gate-records").mkdir()
    for gate, record in records.items():
        path = output / "gate-records" / (gate + ".json")
        if path.exists():
            require(contract.read_json(path, sha(path.read_bytes())) == record, "CHECKPOINT_RECORD_CHANGED")
        else:
            contract.write_new(path, record)
    evidence = {key: manifest[key] for key in ("schema_version", "source_sha", "configuration_sha256", "platform", "images", "producer")}
    evidence["gates"] = {gate: {"status": record["status"],
                                "record_sha256": sha((output / "gate-records" / (gate + ".json")).read_bytes())}
                         for gate, record in records.items()}
    contract.write_new(output / "evidence.json", evidence)
    result = copy.deepcopy(manifest)
    result["evidence"] = contract.file_record(output / "evidence.json", output)
    contract.copy_verified(input_path.parent / "runtime.tar", output / "runtime.tar",
                           {key: manifest["runtime_bundle"][key] for key in ("path", "bytes", "sha256")},
                           limit=oci.MAX_BUNDLE_BYTES)
    contract.write_new(output / "manifest.json", result)
    if all(record["status"] == "PASS" for record in records.values()):
        # Check our own emitted contract, without manufacturing an operator admission.
        oci.verify_evidence(output / "evidence.json", result)
    summary = {"kind": "cantos-native-oci-qualification", "input_manifest_sha256": input_digest,
               "manifest_sha256": sha((output / "manifest.json").read_bytes()),
               "source_sha": manifest["source_sha"], "images": manifest["images"],
               "fixture_configuration_sha256": contract.fingerprint(configuration),
               "fixture_configuration": configuration, "release_eligible": False, "admission": "NOT_RUN",
               "activation": "NOT_RUN", "failure": failure,
               "gates": {gate: record["status"] for gate, record in records.items()}}
    contract.write_new(output / "qualification.json", summary)
    return summary


def qualify(root, args):
    manifest, plan = validate_input(root, args)
    configuration = fixture_configuration(root)
    common = {key: manifest[key] for key in ("source_sha", "configuration_sha256", "platform", "images")}
    common.update(fixture_configuration_sha256=contract.fingerprint(configuration),
                  input_manifest_sha256=args.manifest_digest, command="cargo xtask release oci-qualify",
                  runner={"system": platform.system(), "machine": platform.machine(), "python": platform.python_version()},
                  started_at=int(time.time()))
    records = {gate: {**common, "gate": gate, "status": "NOT_RUN", "reason": "not executed"} for gate in oci.GATES}
    args.output.mkdir(parents=True, exist_ok=False, mode=0o700)
    (args.output / "gate-records").mkdir()
    contract.write_new(args.output / "attempt.json", {**common, "status": "IN_PROGRESS", "release_eligible": False,
                                                   "fixture_configuration": configuration})
    failure = None
    current = "image-pair"
    try:
        require(native_host() == args.platform, "NATIVE_PLATFORM_MISMATCH")
        for tool in ("podman", "nsenter", "ip"):
            require(shutil.which(tool) is not None, "PREREQUISITE_" + tool)
        with tempfile.TemporaryDirectory(prefix="cantos-oci-qualification-") as temporary:
            fixture = Fixture(root, Path(temporary), manifest, args.authfile)
            try:
                for gate, details in run_gates(fixture, manifest, plan):
                    records[gate] = {**common, "gate": gate, "status": "PASS", "evidence_level": "integration-tested",
                                     "execution_environment": "linux-native-exact-images", **details}
                    contract.write_new(args.output / "gate-records" / (gate + ".json"), records[gate])
                    remaining = [g for g in ("image-pair", "native-e2e", "native-isolation", "migration-restore")
                                 if records[g]["status"] != "PASS"]
                    current = remaining[0] if remaining else None
            finally:
                fixture.close()
    except Exception as error:
        safe = isinstance(error, (ValueError, registry.ReadbackError)) and re.fullmatch(r"[A-Za-z0-9_:/.-]{1,256}", str(error))
        failure = str(error) if safe else type(error).__name__
        unavailable = failure.startswith("PREREQUISITE_") or failure in (
            "LINUX_NATIVE_REQUIRED", "ROOTFUL_DISPOSABLE_LINUX_REQUIRED", "NATIVE_PLATFORM_MISMATCH")
        if current and not unavailable:
            records[current] = {**common, "gate": current, "status": "FAIL", "reason": failure}
        else:
            for record in records.values():
                record["reason"] = failure
    return write_result(args.output, manifest, args.manifest, args.manifest_digest, records, configuration, failure)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--manifest", type=Path, required=True)
    for name in ("manifest-digest", "source-sha", "configuration-digest", "platform"):
        parser.add_argument("--" + name, required=True)
    parser.add_argument("--authfile", type=Path)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    try:
        result = qualify(Path(__file__).resolve().parents[1], args)
    except Exception as error:
        parser.exit(1, "OCI qualification input refused: " + type(error).__name__ + "\n")
    print(json.dumps(result, sort_keys=True))
    return 0 if all(v == "PASS" for v in result["gates"].values()) else 1


if __name__ == "__main__":
    sys.exit(main())
