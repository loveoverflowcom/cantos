"""Implementation primitives for xtask release and artifact deploy. No implicit CI trigger."""

import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import tarfile
import time
import uuid

import release_artifact as contract
import release_build


def run(root, argv, *, capture=False, env=None):
    return release_build.run(root, argv, capture=capture, env=env)


def refuse_legacy(component):
    contract.require(component not in ("api", "all"),
                     "legacy api/all release is retired; use native OCI or explicit Web components")


def components(component, build_only):
    refuse_legacy(component)
    choices = {"theatre-web": ["theatre-web"], "studio-web": ["studio-web"],
               "web": ["theatre-web", "studio-web"]}
    if build_only:
        choices["android"] = ["android"]
    contract.require(component in choices, "invalid component selection")
    return choices[component]


def resolve(root, target, component, origin, build_only):
    contract.require(bool(target) != build_only, "use build-only with an empty target, or a release target without build-only")
    selected = components(component, build_only)
    checkout_sha = release_build.source_revision(root)
    config_sha = contract.config_identity()
    producer = contract.ci_identity()
    contract.require(os.environ.get("CIRCLE_BRANCH") == "develop" and os.environ.get("CIRCLE_SHA1") == checkout_sha,
                     "release requests require the reviewed develop config revision")
    origin = contract.api_origin(origin)
    fetch(root)
    run(root, ["git", "merge-base", "--is-ancestor", config_sha, "refs/remotes/origin/develop"])
    if build_only:
        source = checkout_sha
    else:
        target = target.strip().removeprefix("refs/heads/").removeprefix("origin/")
        contract.require(target and not target.startswith("-"), "branch or commit required")
        run(root, ["git", "check-ref-format", "refs/heads/" + target])
        # A branch is resolved only HERE. Downstream consumers use the full SHA.
        exists = subprocess.run(["git", "show-ref", "--verify", "--quiet", "refs/remotes/origin/" + target], cwd=root).returncode == 0
        contract.require(exists or re.fullmatch(r"[0-9a-fA-F]{7,40}", target), "unknown target branch/commit")
        ref = "refs/remotes/origin/" + target if exists else target.lower()
        source = contract.sha(run(root, ["git", "rev-parse", "--verify", ref + "^{commit}"], capture=True))
    run(root, ["git", "merge-base", "--is-ancestor", source, "refs/remotes/origin/develop"])
    return {"schema_version": 1, "repository": contract.REPOSITORY, "source_sha": source,
            "config_sha": config_sha, "initial_checkout_sha": checkout_sha,
            "requested_target": target, "components": selected,
            "api_origin": origin, "producer": producer, "build_only": build_only, "created_at": time.time()}


def fetch(root):
    shallow = run(root, ["git", "rev-parse", "--is-shallow-repository"], capture=True) == "true"
    run(root, ["git", "fetch", "--prune", "--no-tags", *(["--unshallow"] if shallow else []),
               "origin", "+refs/heads/*:refs/remotes/origin/*"])


def one(transfer, name):
    paths = list(transfer.rglob(name))
    contract.require(len(paths) == 1, "workspace must contain exactly one " + name + "; start a new pipeline after ambiguous reruns")
    return paths[0]


def request_from(transfer):
    path = one(transfer, "request.json")
    request = contract.read_json(path)
    contract.check_request(request)
    return path, request


def prepare(root, args):
    request = resolve(root, args.target, args.component, args.api_origin, args.build_only)
    transfer = args.transfer.absolute()
    contract.require(not transfer.exists(), "fresh run-owned transfer directory required")
    attempt = transfer / uuid.uuid4().hex
    attempt.mkdir(parents=True)
    contract.write_new(attempt / "request.json", request)
    run(root, ["git", "checkout", "--detach", request["source_sha"]])
    release_build.source_revision(root, request)
    # Reuse each affected verification suite once for the exact immutable source.
    jobs = ["repository"]
    if "api" in request["components"]:
        jobs.append("rust")
    if any(c.endswith("-web") for c in request["components"]):
        jobs.append("web")
    for job in jobs:
        run(root, ["cargo", "xtask", "ci", "--job", job])
    inventory = {}
    for component in request["components"]:
        if component.endswith("-web"):
            # Only a CI-owned verification output; never a caller-provided dist.
            dist = root / f"apps/cantos-{component[:-4]}/web/dist"
            if dist.exists():
                contract.require(not dist.is_symlink(), "symlink verification dist")
                shutil.rmtree(dist)
        path = release_build.package(root, component, origin=request["api_origin"], request=request, base=attempt / "artifacts")
        inventory[component] = {"manifest": path.relative_to(attempt).as_posix(),
                                "digest": contract.file_record(path, path.parent)["sha256"]}
    contract.write_new(attempt / "inventory.json", {"schema_version": 1,
        "request_sha256": contract.fingerprint(request), "components": inventory})


def checkout(root, transfer):
    release_build.source_revision(root)  # actual config checkout must equal CIRCLE_SHA1
    _, request = request_from(transfer)
    fetch(root)
    run(root, ["git", "merge-base", "--is-ancestor", request["source_sha"], "refs/remotes/origin/develop"])
    run(root, ["git", "checkout", "--detach", request["source_sha"]])
    release_build.source_revision(root, request)


def validated(args, component, origin):
    return contract.validate(args.release_manifest, args.release_digest, component, args.release_sha, origin,
                             request_path=args.release_request, local=args.local_artifact)


def extract_web(root, args):
    manifest = validated(args, args.component, args.api_origin)
    contract.require(manifest["configuration"] == contract.configuration(root, args.component, args.api_origin),
                     "checkout build policy differs from release configuration")
    if args.release_request:
        release_build.source_revision(root, contract.read_json(args.release_request))
    contract.extract(args.release_manifest.parent / (args.component + ".tar.gz"), args.output,
                     web=True, expected=manifest["artifacts"][0])


def assert_fresh(root, request):
    # Restrictive, explicit rollout policy: an approved older target may be built/inspected,
    # but live promotion stops once develop moves. This also fences an old multi-app release
    # after a newer pipeline starts publishing. No deployment lease or distributed transaction.
    fetch(root)
    current = run(root, ["git", "rev-parse", "refs/remotes/origin/develop"], capture=True)
    contract.require(current == request["source_sha"], "stale release: develop moved; prepare/approve a new pipeline")


def promotion_input(root, transfer, component):
    refuse_legacy(component)
    request_path, request = request_from(transfer)
    contract.require("api" not in request["components"], "legacy API transfer is retired")
    contract.require(not request["build_only"] and component in request["components"] and component != "android",
                     "component was not approved for promotion")
    release_build.source_revision(root, request)
    inventory_path = one(transfer, "inventory.json")
    inventory = contract.read_json(inventory_path)
    contract.require(inventory["schema_version"] == 1 and inventory["request_sha256"] == contract.fingerprint(request)
                     and set(inventory["components"]) == set(request["components"]), "foreign/incomplete inventory")
    item = inventory["components"][component]
    contract.safe_name(item["manifest"])
    manifest = inventory_path.parent / item["manifest"]
    origin = None if component == "api" else request["api_origin"]
    contract.validate(manifest, item["digest"], component, request["source_sha"], origin, request_path=request_path)
    assert_fresh(root, request)
    for previous in request["components"][:request["components"].index(component)]:
        prior = contract.read_json(transfer / "outcomes" / previous / "outcome.json")
        contract.require(prior["pipeline"] == request["producer"] and prior["requested_sha"] == request["source_sha"]
                         and prior["manifest_sha256"] == inventory["components"][previous]["digest"]
                         and prior["outcome"] == "activated" and prior["smoke"] == "PASS"
                         and prior["command_status"] == "PASS",
                         "preceding component was not confirmed healthy in this release")
    return request_path, request, item, manifest


def intent(root, transfer, component):
    _, request, item, _ = promotion_input(root, transfer, component)
    directory = transfer / "intents" / component
    contract.require(not directory.exists(), "component intent already exists; reconcile before a new pipeline")
    directory.mkdir(parents=True)
    contract.write_new(directory / "intent.json", {
        "request_sha256": contract.fingerprint(request), "manifest_sha256": item["digest"],
        "publisher": contract.ci_identity(), "component": component})
    print("Prepared component intent; CircleCI must persist this intent before promotion")


def promote(root, transfer, component):
    request_path, request, item, manifest = promotion_input(root, transfer, component)
    planned = contract.read_json(transfer / "intents" / component / "intent.json")
    contract.require(planned == {"request_sha256": contract.fingerprint(request),
                     "manifest_sha256": item["digest"], "publisher": contract.ci_identity(), "component": component},
                     "foreign or rerun publisher intent; reconcile before a new pipeline")
    outcome_dir = transfer / "outcomes" / component
    # Rerunning any attempted write is reconciliation, never an automatic retry.
    contract.require(not outcome_dir.exists(), "component promotion already attempted; reconcile before a new pipeline")
    outcome_dir.mkdir(parents=True)
    outcome = {"schema_version": 1, "component": component, "requested_sha": request["source_sha"],
               "manifest_sha256": item["digest"], "pipeline": request["producer"],
               "publisher": contract.ci_identity(),
               "outcome": "unknown", "smoke": "NOT_RUN", "remote_identity": None, "command_status": "FAIL"}
    artifact = ["--release-manifest", str(manifest), "--release-digest", item["digest"],
                "--release-sha", request["source_sha"], "--release-request", str(request_path)]
    hosting_before = set((root / "artifacts/hosting").glob("*/receipt.json"))
    api_before = set((root / "artifacts/vps-release").glob("*/receipt.json"))
    try:
        if component == "api":
            host = os.environ["VPS_USER"] + "@" + os.environ["VPS_HOST"]
            argv = ["cargo", "xtask", "deploy", "vps", "cantos-server", "--host", host,
                    "--smoke-origin", request["api_origin"], *artifact]
        else:
            # Web-only releases require a healthy compatible API, never an implicit API deploy.
            run(root, ["curl", "--proto", "=https", "--fail", "--silent", "--show-error", "--max-time", "20",
                       "--output", "/dev/null", request["api_origin"] + "/api/v1/theatre/works"])
            app = "cantos-" + component[:-4]
            argv = ["cargo", "xtask", "deploy", "firebase", app, "--web", "--project", os.environ["FIREBASE_PROJECT"],
                    "--channel", "live", *artifact]
        env = dict(os.environ, CANTOS_API_ORIGIN=request["api_origin"])
        run(root, argv, env=env)
        outcome["command_status"] = "PASS"
    finally:
        if component == "api":
            receipts = set((root / "artifacts/vps-release").glob("*/receipt.json")) - api_before
            if len(receipts) == 1:
                receipt = contract.read_json(receipts.pop())
                outcome["api_receipt"] = receipt
                outcome["outcome"] = receipt["activation"]
                outcome["smoke"] = receipt["smoke"]
                outcome["remote_identity"] = {"source_sha": receipt["requested_sha"], "image_id": receipt["image_id"],
                                              "incoming": receipt.get("incoming")}
        else:
            receipts = set((root / "artifacts/hosting").glob("*/receipt.json")) - hosting_before
            if len(receipts) == 1:
                # Preserve the adapter's exact remote outcome separately from command/smoke failure.
                receipt = contract.read_json(receipts.pop())
                outcome["hosting_receipt"] = receipt
                outcome["remote_identity"] = receipt.get("remote")
                outcome["outcome"] = "activated" if receipt["remote"]["state"] == "confirmed" else receipt["remote"]["state"]
                outcome["smoke"] = (receipt.get("smoke") or {}).get("state", "NOT_RUN").upper()
        contract.write_new(outcome_dir / "outcome.json", outcome)


def pages_argv(component, manifest, destination):
    contract.require(component in ("theatre-web", "studio-web"), "one Web component required")
    contract.require(set(destination) == {"account", "project", "branch", "environment"},
                     "exact Pages destination required")
    return ["cargo", "xtask", "deploy", "cloudflare", "cantos-" + component[:-4], "--web",
            *[part for key in ("account", "project", "branch", "environment")
              for part in ("--" + key, destination[key])], "--artifact", str(manifest.parent)]


def pages_intent(root, transfer, component):
    """Bind a reviewed same-workflow Web artifact and explicit destination before workspace persistence."""
    _, request, item, manifest = promotion_input(root, transfer, component)
    directory = transfer / "intents" / component
    contract.require(not directory.exists(), "component intent already exists; reconcile before a new pipeline")
    # Nonsecret operator mapping stays in infra; the deploy owner checks its exact app/branch policy.
    branch = os.environ["CLOUDFLARE_PRODUCTION_BRANCH"]
    environment = os.environ["WEB_ENVIRONMENT"]
    selected_branch = os.environ.get("WEB_BRANCH") or (branch if environment == "production" else "")
    projects = {app: os.environ["CLOUDFLARE_" + app.upper() + "_PROJECT"] for app in ("theatre", "studio")}
    destination = {"account": os.environ["CLOUDFLARE_ACCOUNT_ID"], "project": projects[component[:-4]],
                   "branch": selected_branch, "environment": environment}
    mapping = {"schema_version": 1, "accounts": {destination["account"]: {
        app: {"project": project, "production_branch": branch} for app, project in projects.items()}}}
    path = root / "infra/cloudflare/projects.json"
    path.parent.mkdir(parents=True, exist_ok=True)
    # Never overwrite an operator's existing mapping, even inside CI.
    if path.exists():
        contract.require(contract.read_json(path) == mapping, "existing Pages mapping differs from intent")
    else:
        contract.write_new(path, mapping)
    argv = pages_argv(component, manifest, destination)
    run(root, [*argv, "--dry-run"])
    directory.mkdir(parents=True, mode=0o700)
    contract.write_new(directory / "intent.json", {"schema_version": 1, "provider": "cloudflare",
        "request_sha256": contract.fingerprint(request), "manifest_sha256": item["digest"],
        "publisher": contract.ci_identity(), "component": component, "destination": destination})
    print("Prepared Pages intent; persist it to this workflow before promotion")


def pages_promote(root, transfer, component):
    """Invoke the existing Pages publisher once; a rerun must reconcile its earlier job's intent."""
    _, request, item, manifest = promotion_input(root, transfer, component)
    planned = contract.read_json(transfer / "intents" / component / "intent.json")
    destination = planned.get("destination", {})
    contract.require(planned == {"schema_version": 1, "provider": "cloudflare",
        "request_sha256": contract.fingerprint(request), "manifest_sha256": item["digest"],
        "publisher": contract.ci_identity(), "component": component, "destination": destination},
        "foreign or rerun Pages intent; reconcile before a new pipeline")
    argv = pages_argv(component, manifest, destination)
    outcome_dir = transfer / "outcomes" / component
    contract.require(not outcome_dir.exists(), "Pages promotion already attempted; reconcile before a new pipeline")
    outcome_dir.mkdir(parents=True, mode=0o700)
    outcome = {"schema_version": 1, "provider": "cloudflare", "component": component,
               "requested_sha": request["source_sha"], "manifest_sha256": item["digest"],
               "pipeline": request["producer"], "publisher": contract.ci_identity(), "destination": destination,
               "outcome": "unknown", "smoke": "NOT_RUN", "command_status": "FAIL", "remote_identity": None}
    # A crash leaves a durable unknown marker; no subsequent invocation can repeat the write.
    contract.write_new(outcome_dir / "attempt.json", outcome)
    before = set((root / "artifacts/cloudflare").glob("*/receipt.json"))
    try:
        run(root, argv)
        outcome["command_status"] = "PASS"
    finally:
        receipts = set((root / "artifacts/cloudflare").glob("*/receipt.json")) - before
        if len(receipts) == 1:
            receipt = contract.read_json(receipts.pop())
            artifact = receipt.get("artifact") or {}
            if (receipt.get("app") == "cantos-" + component[:-4] and
                    all(receipt.get(key) == value for key, value in destination.items()) and
                    artifact.get("source_sha") == request["source_sha"] and
                    artifact.get("manifest_sha256") == item["digest"]):
                outcome["pages_receipt"] = receipt
                outcome["remote_identity"] = receipt.get("remote")
                outcome["outcome"] = (receipt.get("remote") or {}).get("state", "unknown")
                outcome["smoke"] = (receipt.get("smoke") or {}).get("state", "NOT_RUN").upper()
            else:
                outcome["receipt_status"] = "FOREIGN_OR_INCOMPLETE"
        contract.write_new(outcome_dir / "outcome.json", outcome)
    contract.require(outcome["outcome"] == "confirmed" and outcome["smoke"] == "PASS",
                     "Pages outcome or smoke unconfirmed; reconcile before a new pipeline")


def report(transfer):
    _, request = request_from(transfer)
    rows = {}
    for component in request["components"]:
        paths = list((transfer / "outcomes" / component).glob("outcome.json"))
        rows[component] = contract.read_json(paths[0]) if len(paths) == 1 else {"outcome": "NOT_RUN/unknown", "smoke": "NOT_RUN"}
        print(component + ": " + rows[component]["outcome"] + "; smoke=" + rows[component]["smoke"])
    summary = {"requested_sha": request["source_sha"], "components": rows,
               "browser_acceptance": "NOT_RUN", "atomic_release": False}
    path = transfer / ("summary-" + uuid.uuid4().hex + ".json")
    contract.write_new(path, summary)
    contract.require(all(row["outcome"] == "activated" and row["smoke"] == "PASS"
                         and row.get("command_status") == "PASS" for row in rows.values()),
                     "partial or unconfirmed release: reconcile each component; never replay all writes")


def inspect_image(root, archive_path, image, revision, destination):
    # Decompress with a bounded output, then inspect the actual docker-archive config.
    raw = destination / "image.tar"
    with raw.open("xb") as handle:
        process = subprocess.Popen(["zstd", "-dc", "--memory=256MB", str(archive_path)], cwd=root, stdout=subprocess.PIPE)
        try:
            total = 0
            while chunk := process.stdout.read(1024 * 1024):
                total += len(chunk)
                contract.require(total <= contract.MAX_BYTES, "image archive exceeds release limit")
                handle.write(chunk)
            contract.require(process.wait() == 0, "truncated/corrupt zstd image")
        finally:
            if process.poll() is None:
                process.kill()
            process.wait()
            process.stdout.close()
    inspect_docker_archive(raw, image, revision)


def inspect_docker_archive(raw, image, revision):
    with tarfile.open(raw) as archive:
        entries = contract.archive_members(archive, image=True)
        names = {e.name for e in entries}
        contract.require("manifest.json" in names, "missing docker image manifest")
        descriptor = json.loads(archive.extractfile("manifest.json").read(contract.MAX_JSON))
        contract.require(len(descriptor) == 1 and descriptor[0]["RepoTags"] == ["localhost/cantos-api:" + revision], "wrong image tag")
        config_path = contract.safe_name(descriptor[0]["Config"])
        layers = descriptor[0]["Layers"]
        contract.require(isinstance(layers, list) and len(layers) <= 100, "invalid image layers")
        expected = {"manifest.json", config_path, *layers}
        # Docker archives may include repositories and per-layer VERSION/json metadata.
        extras = {"repositories"} | {str(Path(layer).parent / n) for layer in layers for n in ("VERSION", "json")}
        contract.require(expected <= names and names <= expected | extras, "unexpected image archive files")
        config_bytes = archive.extractfile(config_path).read(contract.MAX_JSON)
        config = json.loads(config_bytes)
        contract.require("sha256:" + hashlib.sha256(config_bytes).hexdigest() == image["id"], "image ID mismatch")
        contract.require(config["architecture"] == "amd64" and config["os"] == "linux" and
                         config["config"]["Labels"]["org.opencontainers.image.revision"] == revision and
                         config["config"]["Labels"]["io.cantos.schema-epoch"] == "2", "image source/platform/schema mismatch")
        # Verify uncompressed layer content against the config, not just a caller's image ID.
        diff_ids = config["rootfs"]["diff_ids"]
        contract.require(len(layers) == len(diff_ids), "wrong image layer count")
        for layer, expected_digest in zip(layers, diff_ids):
            with archive.extractfile(layer) as handle:
                actual = hashlib.file_digest(handle, "sha256").hexdigest()
            contract.require("sha256:" + actual == expected_digest, "corrupt image layer")


def deploy_api(root, args):
    raise ValueError("legacy VPS API deployment is retired; native host activation is not implemented")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    actions = parser.add_subparsers(dest="action", required=True)
    prepare_parser = actions.add_parser("prepare")
    prepare_parser.add_argument("--target")
    prepare_parser.add_argument("--build-only", action="store_true")
    prepare_parser.add_argument("--component", required=True)
    prepare_parser.add_argument("--api-origin", required=True)
    for name in ("checkout", "intent", "promote", "report", "pages-intent", "pages-promote"):
        child = actions.add_parser(name)
        child.add_argument("--transfer", type=Path, required=True)
        if name in ("intent", "promote", "pages-intent", "pages-promote"):
            child.add_argument("--component", required=True)
    prepare_parser.add_argument("--transfer", type=Path, required=True)
    for name in ("extract", "deploy-api"):
        child = actions.add_parser(name)
        child.add_argument("--release-manifest", type=Path, required=True)
        child.add_argument("--release-digest", required=True)
        child.add_argument("--release-sha", required=True)
        child.add_argument("--release-request", type=Path)
        child.add_argument("--local-artifact", action="store_true")
        child.add_argument("--api-origin", required=True)
        if name == "extract":
            child.add_argument("--component", choices=["theatre-web", "studio-web"], required=True)
            child.add_argument("--output", type=Path, required=True)
        else:
            child.add_argument("--host", required=True)
            child.add_argument("--dry-run", action="store_true")
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[1]
    if args.action == "prepare":
        prepare(root, args)
    elif args.action == "checkout":
        checkout(root, args.transfer)
    elif args.action == "promote":
        promote(root, args.transfer, args.component)
    elif args.action == "intent":
        intent(root, args.transfer, args.component)
    elif args.action == "pages-intent":
        pages_intent(root, args.transfer, args.component)
    elif args.action == "pages-promote":
        pages_promote(root, args.transfer, args.component)
    elif args.action == "report":
        report(args.transfer)
    elif args.action == "extract":
        extract_web(root, args)
    else:
        deploy_api(root, args)


if __name__ == "__main__":
    main()
