"""Federated registry contracts over temporary repositories, plus the real registry's shape.

Fixtures copy real Quint files to module-owned locations, so inline and external-manifest
capabilities are exercised through the same loader, snapshot and impact index the gate uses.
The real registry is checked against the layout rule: a model lives in its owner's boundary.
"""
import copy
import json
import posixpath
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

import behavior_registry as registry
import verify_behavior as behavior

FORGE = "cantos.production.forge-attempt"
HOSTING = "cantos.tooling.hosting-deploy"
SERVICE = "services/demo/behavior"
CLOCK = "specs/behavior/shared/clock.qnt"
CLOCK_FROM_SERVICE = "../../../specs/behavior/shared/clock"


def real_catalog():
    """The real registry with every manifest pointer resolved back to an inline record.

    Fixtures start from the inline form (and `federate` turns a record into a pointer again), so
    they stay independent of where the real repository keeps each manifest.
    """
    catalog = behavior.read_json(behavior.ROOT / behavior.CATALOG)
    catalog["capabilities"] = [{k: v for k, v in cap.items() if k != "manifestPath"}
                               for cap in behavior.resolve_capabilities(behavior.ROOT, catalog)]
    return catalog


def real_capability(identity):
    return copy.deepcopy(next(c for c in real_catalog()["capabilities"] if c["id"] == identity))


def federate(root, identity, manifest="specs/behavior/federated.manifest.json"):
    """Replace one inline record in a temporary catalog by a pointer plus its manifest."""
    catalog = json.loads((root / behavior.CATALOG).read_text())
    for index, cap in enumerate(catalog["capabilities"]):
        if cap["id"] == identity:
            payload = {k: v for k, v in cap.items() if k not in registry.AUTHORITY_KEYS}
            pointer = {k: cap[k] for k in ("id", "owner", "lifecycle")} | {"manifestPath": manifest}
            catalog["capabilities"][index] = pointer
            target = root / manifest
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_text(json.dumps({"schemaVersion": 1, "capabilityId": identity} | payload))
    (root / behavior.CATALOG).write_text(json.dumps(catalog))


class Repo:
    def __init__(self, case):
        directory = tempfile.TemporaryDirectory(prefix="cantos-registry-")
        case.addCleanup(directory.cleanup)
        self.root = Path(directory.name)

    def put(self, relative, text="placeholder\n"):
        target = self.root / relative
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_text(text)
        return target

    def copy(self, source, target, edit=None):
        text = (behavior.ROOT / source).read_text()
        self.put(target, edit(text) if edit else text)

    def edit_catalog(self, change):
        path = self.root / behavior.CATALOG
        catalog = json.loads(path.read_text())
        change(catalog)
        path.write_text(json.dumps(catalog))

    def load(self, selected=None):
        return behavior.load_catalog(self.root, selected)

    def registry(self):
        return behavior.load_registry(self.root)


def registry_module():
    return behavior.REGISTRY_MODULE


def references(repo, cap):
    for relative in [cap["owner"], cap["inventory"], *[r.split("#", 1)[0] for r in cap["sourceRefs"]]]:
        if not (repo.root / relative).exists():
            repo.put(relative)


def build(case, *, helper=False, manifest=True):
    """A module-owned forge capability beside an inline hosting capability."""
    repo = Repo(case)
    for relative in ("scripts/verify_behavior.py", registry_module(), "scripts/verify_kani.py", "xtask/src/behavior.rs",
                     "xtask/src/gate.rs", "xtask/src/cli.rs", "Cargo.lock", "tools/behavior/package.json"):
        repo.put(relative)
    forge, hosting = real_capability(FORGE), real_capability(HOSTING)
    import_line = f'module forge_attempt {{\n  import clock.* from "{CLOCK_FROM_SERVICE}"' if helper else None
    repo.copy(forge["model"], f"{SERVICE}/forge_attempt.qnt",
              (lambda t: t.replace("module forge_attempt {", import_line, 1)) if helper else None)
    repo.copy(forge["scenarios"], f"{SERVICE}/forge_attempt_tests.qnt")
    forge.update(model=f"{SERVICE}/forge_attempt.qnt", scenarios=f"{SERVICE}/forge_attempt_tests.qnt")
    for relative in (hosting["model"], hosting["scenarios"]):
        repo.copy(relative, relative)
    for cap in (forge, hosting):
        references(repo, cap)
    if helper:
        repo.put(CLOCK, "module clock {\n  pure def tick(n: int): int = n + 1\n}\n")
    catalog = {"schemaVersion": 1, "capabilities": [forge, hosting]}
    if helper:
        catalog["sharedHelpers"] = [{"path": CLOCK, "reason": "stateless clock arithmetic"}]
    repo.put(behavior.CATALOG, json.dumps(catalog))
    if manifest:
        federate(repo.root, FORGE, f"{SERVICE}/forge.manifest.json")
    return repo


def rejects(case, repo, message=None):
    with case.assertRaises(behavior.BehaviorError) as caught:
        repo.registry()
    if message:
        case.assertIn(message, str(caught.exception))


class RealCatalogCompatibility(unittest.TestCase):
    def test_every_capability_resolves_to_one_normalized_shape(self):
        entries = behavior.read_json(behavior.ROOT / behavior.CATALOG)["capabilities"]
        capabilities, helpers = behavior.load_registry(behavior.ROOT)
        self.assertEqual(helpers, [])
        self.assertEqual([c["id"] for c in capabilities], [e["id"] for e in entries])
        self.assertEqual(
            {c["id"] for c in capabilities if c.get("model") is not None},
            {c["id"] for c in behavior.load_catalog(behavior.ROOT)},
        )
        for resolved, entry in zip(capabilities, entries):
            with self.subTest(capability=entry["id"]):
                if resolved.get("model") is None:
                    # Deprecated and unmodeled records stay inline, byte for byte.
                    self.assertEqual({k: v for k, v in resolved.items() if k != "importClosure"}, entry)
                    continue
                self.assertEqual(sorted(entry), ["id", "lifecycle", "manifestPath", "owner"])
                self.assertEqual(posixpath.dirname(entry["manifestPath"]), posixpath.dirname(resolved["model"]))
                self.assertEqual(posixpath.dirname(resolved["scenarios"]), posixpath.dirname(resolved["model"]))
                self.assertEqual(posixpath.dirname(resolved["inventory"]), posixpath.dirname(resolved["model"]))
                self.assertEqual(registry.boundary(resolved["model"]), registry.boundary(resolved["owner"]),
                                 "a model lives in the boundary that owns the behavior")

    def test_the_root_inventory_index_links_every_module_owned_document(self):
        text = (behavior.ROOT / "specs/behavior/inventory.md").read_text()
        for cap in behavior.load_catalog(behavior.ROOT):
            with self.subTest(capability=cap["id"]):
                self.assertIn(f"| `{cap['id']}` |", text)
                self.assertIn("](" + posixpath.relpath(cap["inventory"], "specs/behavior") + ")", text)

    def test_deprecated_and_unmodeled_entries_remain_valid_but_cannot_be_selected(self):
        capabilities, _ = behavior.load_registry(behavior.ROOT)
        by_id = {c["id"]: c for c in capabilities}
        self.assertEqual(by_id["cantos.identity.theatre-registration"]["lifecycle"], "deprecated")
        # The retained historical model is registered through its scenario's import.
        self.assertEqual(by_id["cantos.identity.theatre-registration"]["importClosure"],
                         ["specs/behavior/identity/theatre_registration.qnt"])
        for identity in ("cantos.publishing.lifecycle", "cantos.listening.progress",
                         "cantos.identity.theatre-registration"):
            with self.subTest(identity=identity), self.assertRaises(behavior.BehaviorError):
                behavior.load_catalog(behavior.ROOT, identity)

    def test_current_models_have_no_cross_capability_closure(self):
        for cap in behavior.load_catalog(behavior.ROOT):
            self.assertEqual(cap["importClosure"], [], cap["id"])

    def test_the_index_command_is_read_only_and_names_all_runnable_models(self):
        result = subprocess.run([sys.executable, str(behavior.ROOT / "scripts/verify_behavior.py"), "--impact-index"],
                                capture_output=True, text=True, cwd=behavior.ROOT, check=True)
        index = json.loads(result.stdout)
        self.assertEqual(set(index["capabilities"]), {cap["id"] for cap in behavior.load_catalog(behavior.ROOT)})
        self.assertIn("cantos.media.sample-metadata", index["capabilities"])
        self.assertEqual(index["inputs"]["specs/behavior/identity/theatre_registration_tests.qnt"], ["*"])
        for argument in ("--setup", "--conformance"):
            failed = subprocess.run([sys.executable, str(behavior.ROOT / "scripts/verify_behavior.py"),
                                     "--impact-index", argument], capture_output=True, text=True, cwd=behavior.ROOT)
            self.assertEqual(failed.returncode, 1)

    def test_observed_capabilities_declare_their_conformance_sources(self):
        index = registry.impact_index(*behavior.load_registry(behavior.ROOT))
        self.assertEqual(index["observed"], ["cantos.identity.native-mailbox-proof",
                                             "cantos.identity.native-session-admission",
                                             "cantos.media.private-audio-admission"])
        audio = {p for p, ids in index["conformance"].items() if ids == ["cantos.media.private-audio-admission"]}
        self.assertIn("services/cantos-server/src/contexts/media/quint_conformance_tests.rs", audio)
        self.assertIn("services/cantos-server/src/contexts/media/domain/lifecycle.rs", audio)


class ManifestResolution(unittest.TestCase):
    def test_inline_and_external_manifest_resolve_to_one_normalized_shape(self):
        inline = build(self, manifest=False)
        external = build(self)
        expected = inline.load(FORGE)[0]
        resolved = external.load(FORGE)[0]
        self.assertEqual(resolved["manifestPath"], f"{SERVICE}/forge.manifest.json")
        self.assertEqual({k: v for k, v in resolved.items() if k != "manifestPath"}, expected)
        self.assertEqual([c["id"] for c in external.load()], [FORGE, HOSTING])
        self.assertEqual([c["id"] for c in external.load(HOSTING)], [HOSTING])

    def test_root_keeps_identity_owner_and_lifecycle_authoritative(self):
        repo = build(self)
        self.assertEqual(sorted(json.loads((repo.root / behavior.CATALOG).read_text())["capabilities"][0]),
                         ["id", "lifecycle", "manifestPath", "owner"])
        repo.edit_catalog(lambda c: c["capabilities"][0].update(lifecycle="accepted"))
        self.assertEqual(repo.load(FORGE)[0]["lifecycle"], "accepted")

    def test_duplicated_or_conflicting_authority_fails_closed(self):
        for field, value in (("owner", "services/demo/src/core.rs"), ("lifecycle", "draft"), ("id", FORGE)):
            repo = build(self)
            path = repo.root / SERVICE / "forge.manifest.json"
            manifest = json.loads(path.read_text())
            manifest[field] = value
            path.write_text(json.dumps(manifest))
            with self.subTest(field=field):
                rejects(self, repo, "duplicates root-owned")

    def test_root_pointer_cannot_also_carry_a_verification_payload(self):
        repo = build(self)
        repo.edit_catalog(lambda c: c["capabilities"][0].update(model=f"{SERVICE}/forge_attempt.qnt"))
        rejects(self, repo, "duplicates verification fields")

    def test_malformed_manifests_fail_closed(self):
        edits = {
            "wrong capability": lambda m: m.update(capabilityId="cantos.other"),
            "wrong schema": lambda m: m.update(schemaVersion=2),
            "unknown field": lambda m: m.update(surprise=True),
            "nested pointer": lambda m: m.update(manifestPath="specs/behavior/catalog.json"),
            "missing field": lambda m: m.pop("sourceRefs"),
            "bad budget": lambda m: m["simulation"].update(samples=0),
        }
        for label, edit in edits.items():
            repo = build(self)
            path = repo.root / SERVICE / "forge.manifest.json"
            manifest = json.loads(path.read_text())
            edit(manifest)
            path.write_text(json.dumps(manifest))
            with self.subTest(label=label), self.assertRaises(behavior.BehaviorError):
                repo.load()
        repo = build(self)
        (repo.root / SERVICE / "forge.manifest.json").write_text("{")
        rejects(self, repo, "invalid JSON")
        repo = build(self)
        (repo.root / SERVICE / "forge.manifest.json").write_text("[]")
        rejects(self, repo, "must be an object")

    def test_manifest_paths_are_normalized_repository_files(self):
        for pointer in ("../outside.json", "/etc/passwd", "./x.json", "services//x.json", "a\\b.json",
                        f"{SERVICE}/missing.json", f"{SERVICE}/forge.manifest.json#anchor", "specs/behavior/catalog.json",
                        f"{SERVICE}/forge_attempt.qnt", ""):
            repo = build(self)
            repo.edit_catalog(lambda c: c["capabilities"][0].update(manifestPath=pointer))
            with self.subTest(pointer=pointer):
                rejects(self, repo)

    def test_linked_manifests_and_directories_are_unsafe(self):
        repo = build(self)
        real = repo.root / SERVICE / "forge.manifest.json"
        moved = repo.root / "elsewhere.json"
        real.rename(moved)
        real.symlink_to(moved)
        rejects(self, repo, "linked")
        repo = build(self)
        target = repo.root / "services/demo"
        (repo.root / "services/linked").symlink_to(target, target_is_directory=True)
        repo.edit_catalog(lambda c: c["capabilities"][0].update(manifestPath=f"services/linked/behavior/forge.manifest.json"))
        rejects(self, repo, "linked")

    def test_one_manifest_cannot_serve_two_capabilities(self):
        repo = build(self)
        repo.edit_catalog(lambda c: c["capabilities"].append(
            {"id": "cantos.other.copy", "owner": c["capabilities"][0]["owner"], "lifecycle": "draft",
             "manifestPath": c["capabilities"][0]["manifestPath"]}))
        rejects(self, repo, "more than one capability")

    def test_duplicate_capability_ids_fail_across_inline_and_external_forms(self):
        repo = build(self)
        repo.edit_catalog(lambda c: c["capabilities"].append(copy.deepcopy(real_capability(HOSTING))))
        rejects(self, repo, "duplicate capability ID")

    def test_missing_model_or_scenario_and_escaping_paths_fail(self):
        for key, value in (("model", f"{SERVICE}/gone.qnt"), ("scenarios", f"{SERVICE}/gone.qnt"),
                           ("model", "../outside.qnt"), ("model", f"{SERVICE}/../../x.qnt")):
            repo = build(self)
            path = repo.root / SERVICE / "forge.manifest.json"
            manifest = json.loads(path.read_text())
            manifest[key] = value
            path.write_text(json.dumps(manifest))
            with self.subTest(key=key, value=value), self.assertRaises(behavior.BehaviorError):
                repo.load()

    def test_changing_a_registered_model_changes_only_that_capability_snapshot(self):
        repo = build(self)
        forge, hosting = repo.load()
        before = behavior.snapshot(repo.root, [forge])
        (repo.root / forge["model"]).write_text((repo.root / forge["model"]).read_text() + "\n// edited\n")
        after = behavior.snapshot(repo.root, [forge])
        self.assertNotEqual(before[forge["model"]], after[forge["model"]])
        self.assertEqual(behavior.snapshot(repo.root, [hosting]), behavior.snapshot(repo.root, [hosting]))
        self.assertNotIn(hosting["model"], after)
        self.assertIn(forge["manifestPath"], after)


class SourceDiscovery(unittest.TestCase):
    def test_orphan_executable_models_fail_wherever_a_module_keeps_them(self):
        for relative in (f"{SERVICE}/orphan.qnt", "specs/behavior/stray/orphan.qnt",
                         "apps/demo/behavior/orphan.qnt", "xtask/behavior/orphan.qnt"):
            repo = build(self)
            repo.put(relative, "module orphan {}\n")
            with self.subTest(relative=relative):
                rejects(self, repo, "unregistered executable Quint file")

    def test_generated_and_dependency_trees_are_not_scanned(self):
        repo = build(self)
        for relative in ("services/demo/target/x.qnt", "apps/demo/node_modules/x.qnt", "apps/demo/build/x.qnt"):
            repo.put(relative, "module generated {}\n")
        repo.registry()

    def test_registered_historical_scenarios_and_their_models_are_not_orphans(self):
        repo = build(self)
        repo.copy("specs/behavior/identity/theatre_registration.qnt", "specs/behavior/identity/theatre_registration.qnt")
        repo.copy("specs/behavior/identity/theatre_registration_tests.qnt", "specs/behavior/identity/theatre_registration_tests.qnt")
        history = real_capability("cantos.identity.theatre-registration")
        history["owner"] = history["inventory"] = history["sourceRefs"][0]
        references(repo, history)
        repo.edit_catalog(lambda c: c["capabilities"].append(history))
        capabilities, _ = repo.registry()
        self.assertEqual(capabilities[-1]["importClosure"], ["specs/behavior/identity/theatre_registration.qnt"])
        repo.edit_catalog(lambda c: c["capabilities"].pop())
        rejects(self, repo, "unregistered executable Quint file")

    def test_a_file_cannot_be_registered_by_two_capabilities(self):
        repo = build(self)
        clone = json.loads(json.dumps(real_capability(HOSTING)).replace(HOSTING, "cantos.other.clone"))
        repo.edit_catalog(lambda c: c["capabilities"].append(clone))
        rejects(self, repo, "registered twice")

    def test_unmodeled_entries_still_need_a_reason(self):
        repo = build(self)
        repo.edit_catalog(lambda c: c["capabilities"].append(
            {"id": "cantos.later.thing", "lifecycle": "needs-decision", "owner": f"{SERVICE}/../src/core.rs",
             "sourceRefs": ["specs/behavior/inventory.md"], "model": None, "inventory": "specs/behavior/inventory.md"}))
        with self.assertRaises(behavior.BehaviorError):
            repo.load()
        repo.put("services/demo/src/core.rs")
        repo.edit_catalog(lambda c: c["capabilities"][-1].update(
            owner="services/demo/src/core.rs", reason="Deferred to its owner."))
        repo.put("specs/behavior/inventory.md")
        repo.registry()


class ImportBoundaries(unittest.TestCase):
    def test_a_registered_stateless_shared_helper_is_allowed_and_enters_the_closure(self):
        repo = build(self, helper=True)
        forge = repo.load(FORGE)[0]
        self.assertEqual(forge["importClosure"], [CLOCK])
        self.assertIn(CLOCK, behavior.snapshot(repo.root, [forge]))

    def test_changing_the_helper_invalidates_evidence_of_every_importer_only(self):
        repo = build(self, helper=True)
        forge, hosting = repo.load()
        before = behavior.snapshot(repo.root, [forge])
        (repo.root / CLOCK).write_text("module clock {\n  pure def tick(n: int): int = n + 2\n}\n")
        self.assertNotEqual(before, behavior.snapshot(repo.root, [forge]))
        self.assertNotIn(CLOCK, behavior.snapshot(repo.root, [hosting]))

    def test_helper_bytes_are_retained_with_the_evidence_bundle(self):
        repo = build(self, helper=True)
        forge = repo.load(FORGE)[0]
        inputs = behavior.snapshot(repo.root, [forge])
        bundle = repo.root / "bundle"
        behavior.retain_inputs(repo.root, bundle, inputs)
        for relative in (CLOCK, forge["manifestPath"], forge["model"], forge["scenarios"]):
            self.assertEqual((bundle / "inputs" / relative).read_bytes(), (repo.root / relative).read_bytes())

    def test_an_unregistered_helper_is_a_cross_boundary_import(self):
        repo = build(self, helper=True)
        repo.edit_catalog(lambda c: c.pop("sharedHelpers"))
        rejects(self, repo, "cross-boundary behavioral import")

    def test_stateful_or_invalid_helpers_are_rejected(self):
        for text in ("module clock {\n  var n: int\n}\n", "module clock {\n  action step = true\n}\n"):
            repo = build(self, helper=True)
            repo.put(CLOCK, text)
            with self.subTest(text=text):
                rejects(self, repo, "stateless")
        repo = build(self, helper=True)
        repo.put("specs/behavior/shared/other.qnt", "module other {}\n")
        repo.put(CLOCK, 'module clock {\n  import other.* from "./other"\n}\n')
        rejects(self, repo, "shared helper imports a non-helper")
        repo = build(self, helper=True)
        repo.edit_catalog(lambda c: c["sharedHelpers"][0].pop("reason"))
        rejects(self, repo, "invalid shared helper record")
        repo = build(self, helper=True)
        repo.edit_catalog(lambda c: c["sharedHelpers"].append(dict(c["sharedHelpers"][0])))
        rejects(self, repo, "duplicate shared helper")

    def test_cross_service_behavioral_imports_are_forbidden(self):
        repo = build(self)
        repo.put("apps/other/behavior/shared.qnt", "module shared {}\n")
        text = (repo.root / SERVICE / "forge_attempt.qnt").read_text()
        (repo.root / SERVICE / "forge_attempt.qnt").write_text(
            text.replace("module forge_attempt {", 'module forge_attempt {\n  import shared.* from "../../../apps/other/behavior/shared"', 1))
        rejects(self, repo, "cross-boundary behavioral import")

    def test_imports_of_missing_nonlocal_or_escaping_files_fail(self):
        for specifier in ("./missing", "bare_module", "../../../../../outside"):
            repo = build(self)
            text = (repo.root / SERVICE / "forge_attempt.qnt").read_text()
            (repo.root / SERVICE / "forge_attempt.qnt").write_text(
                text.replace("module forge_attempt {", f'module forge_attempt {{\n  import x.* from "{specifier}"', 1))
            with self.subTest(specifier=specifier):
                rejects(self, repo)

    def test_import_cycles_fail(self):
        repo = build(self)
        repo.put(f"{SERVICE}/a.qnt", 'module a {\n  import b.* from "./b"\n}\n')
        repo.put(f"{SERVICE}/b.qnt", 'module b {\n  import a.* from "./a"\n}\n')
        text = (repo.root / SERVICE / "forge_attempt.qnt").read_text()
        (repo.root / SERVICE / "forge_attempt.qnt").write_text(
            text.replace("module forge_attempt {", 'module forge_attempt {\n  import a.* from "./a"', 1))
        rejects(self, repo, "cyclic behavior import")

    def test_commented_and_quoted_import_text_is_not_an_import(self):
        repo = build(self)
        text = (repo.root / SERVICE / "forge_attempt.qnt").read_text()
        (repo.root / SERVICE / "forge_attempt.qnt").write_text(
            text.replace("module forge_attempt {",
                         'module forge_attempt {\n  // import x.* from "./missing"\n  /* import y.* from "./gone" */', 1))
        self.assertEqual(repo.load(FORGE)[0]["importClosure"], [])

    def test_a_same_boundary_cross_capability_import_makes_the_importer_depend_on_it(self):
        repo = build(self)
        repo.put(f"{SERVICE}/support.qnt", "module support {}\n")
        text = (repo.root / SERVICE / "forge_attempt_tests.qnt").read_text()
        (repo.root / SERVICE / "forge_attempt_tests.qnt").write_text(
            text.replace("{", '{\n  import support.* from "./support"', 1))
        forge = repo.load(FORGE)[0]
        self.assertEqual(forge["importClosure"], [f"{SERVICE}/support.qnt"])
        index = registry.impact_index(*repo.registry())
        self.assertEqual(index["inputs"][f"{SERVICE}/support.qnt"], [FORGE])


class OwnedSources(unittest.TestCase):
    def meta(self, repo, **fields):
        path = repo.root / SERVICE / "forge.manifest.json"
        manifest = json.loads(path.read_text())
        manifest.update(fields)
        path.write_text(json.dumps(manifest))

    def test_bounded_owned_files_and_directories_are_accepted(self):
        repo = build(self)
        repo.put("services/demo/src/core.rs")
        repo.put("services/demo/src/owned/rule.rs")
        self.meta(repo, ownedSources=["services/demo/src/owned/"], implementationTests=["services/demo/src/core.rs"])
        self.assertEqual(repo.load(FORGE)[0]["ownedSources"], ["services/demo/src/owned/"])

    def test_broad_missing_duplicate_or_unsafe_ownership_is_rejected(self):
        repo = build(self)
        repo.put("services/demo/src/core.rs")
        for value in (["services/"], ["services/demo/"], ["missing.rs"], ["services/demo/src/core.rs"] * 2, [],
                      ["services/demo/src/core.rs#anchor"], "services/demo/src/core.rs", ["../x.rs"],
                      [f"services/demo/{i}.rs" for i in range(65)]):
            self.meta(repo, ownedSources=value)
            with self.subTest(value=str(value)[:40]), self.assertRaises(behavior.BehaviorError):
                repo.load()


class ImpactIndex(unittest.TestCase):
    def index(self, repo):
        return registry.impact_index(*repo.registry())

    def test_models_scenarios_manifests_and_helpers_map_to_their_capabilities(self):
        repo = build(self, helper=True)
        index = self.index(repo)
        self.assertEqual(index["capabilities"], [FORGE, HOSTING])
        for path in (f"{SERVICE}/forge_attempt.qnt", f"{SERVICE}/forge_attempt_tests.qnt", f"{SERVICE}/forge.manifest.json"):
            self.assertEqual(index["inputs"][path], [FORGE])
        self.assertEqual(index["inputs"][real_capability(HOSTING)["model"]], [HOSTING])
        self.assertEqual(index["inputs"][CLOCK], [FORGE])

    def test_an_unused_helper_has_unknown_impact(self):
        repo = build(self, helper=True)
        text = (repo.root / SERVICE / "forge_attempt.qnt").read_text()
        (repo.root / SERVICE / "forge_attempt.qnt").write_text(text.replace(f'  import clock.* from "{CLOCK_FROM_SERVICE}"\n', ""))
        self.assertEqual(self.index(repo)["inputs"][CLOCK], ["*"])

    def test_source_references_select_models_but_documents_never_do(self):
        repo = build(self)
        index = self.index(repo)
        hosting = real_capability(HOSTING)
        code = [hosting["owner"], *[r for r in hosting["sourceRefs"] if r.endswith(".rs")]]
        self.assertGreater(len(code), 1)
        for path in code:
            self.assertEqual(index["sources"][path], [HOSTING])
        self.assertEqual(index["sources"][real_capability(FORGE)["owner"]], [FORGE])
        for table in ("sources", "conformance"):
            self.assertFalse([p for p in index[table] if p.endswith(".md")])
        self.assertNotIn("specs/behavior/inventory.md", index["sources"])

    def test_only_observed_capabilities_get_conformance_triggers(self):
        repo = build(self)
        repo.put("services/demo/src/core.rs")
        path = repo.root / SERVICE / "forge.manifest.json"
        manifest = json.loads(path.read_text())
        manifest.update(ownedSources=["services/demo/src/core.rs"], implementationTests=["services/demo/src/observer.rs"])
        path.write_text(json.dumps(manifest))
        repo.put("services/demo/src/observer.rs")
        index = self.index(repo)
        self.assertEqual(index["conformance"], {})
        self.assertEqual(index["sources"]["services/demo/src/core.rs"], [FORGE])
        manifest.update(observationSchema="demo-observation-v1", implementationConformance="pure-core-observer")
        path.write_text(json.dumps(manifest))
        index = self.index(repo)
        self.assertEqual(index["observed"], [FORGE])
        self.assertEqual(sorted(index["conformance"]), [
            real_capability(FORGE)["owner"], "services/demo/src/core.rs", "services/demo/src/observer.rs"])

    def test_directory_ownership_becomes_a_prefix_trigger(self):
        repo = build(self)
        repo.put("services/demo/src/owned/rule.rs")
        path = repo.root / SERVICE / "forge.manifest.json"
        manifest = json.loads(path.read_text())
        manifest["ownedSources"] = ["services/demo/src/owned/"]
        path.write_text(json.dumps(manifest))
        self.assertEqual(self.index(repo)["sourcePrefixes"], {"services/demo/src/owned/": [FORGE]})

    def test_non_runnable_entries_have_no_scoped_triggers(self):
        index = registry.impact_index(*behavior.load_registry(behavior.ROOT))
        for ids in (*index["sources"].values(), *index["conformance"].values()):
            self.assertNotIn("cantos.publishing.lifecycle", ids)
            self.assertNotIn("cantos.identity.theatre-registration", ids)
        self.assertEqual(index["inputs"]["specs/behavior/identity/theatre_registration.qnt"], ["*"])

    def test_live_unmodeled_owners_derive_obligations_in_the_real_registry(self):
        index = registry.impact_index(*behavior.load_registry(behavior.ROOT))
        self.assertEqual(index["schemaVersion"], 2)
        self.assertEqual(index["obligations"], {
            "services/cantos-server/src/contexts/listening/domain/reconcile.rs": ["cantos.listening.progress"],
            "services/cantos-server/src/contexts/media/domain/publication.rs": ["cantos.publishing.lifecycle"]})
        self.assertEqual(index["unmodeled"]["cantos.listening.progress"]["lifecycle"], "needs-decision")
        # A deprecated record keeps its history but owes no decision.
        self.assertNotIn("cantos.identity.theatre-registration", index["unmodeled"])
        self.assertFalse(set(index["unmodeled"]) & set(index["capabilities"]))

    def test_unmodeled_obligations_follow_the_registry_and_never_select_a_model(self):
        repo = build(self)
        catalog = json.loads((repo.root / behavior.CATALOG).read_text())
        live = real_capability("cantos.publishing.lifecycle")
        live.update(sourceRefs=[*live["sourceRefs"], "services/demo/src/rule.rs#decision"],
                    ownedSources=["services/demo/src/owned/"])
        retired = dict(live, id="cantos.publishing.retired", lifecycle="deprecated",
                       owner="services/demo/src/retired.rs", sourceRefs=["services/demo/src/retired.rs"])
        retired.pop("ownedSources")
        for relative in ("services/demo/src/rule.rs", "services/demo/src/owned/core.rs", "services/demo/src/retired.rs"):
            repo.put(relative)
        for cap in (live, retired):
            references(repo, cap)
        catalog["capabilities"] += [live, retired]
        repo.put(behavior.CATALOG, json.dumps(catalog))
        index = self.index(repo)
        self.assertEqual(sorted(index["unmodeled"]), ["cantos.publishing.lifecycle"])
        self.assertEqual(index["obligations"], {
            live["owner"]: ["cantos.publishing.lifecycle"],
            "services/demo/src/rule.rs": ["cantos.publishing.lifecycle"]})
        self.assertEqual(index["obligationPrefixes"], {"services/demo/src/owned/": ["cantos.publishing.lifecycle"]})
        for table in ("obligations", "obligationPrefixes"):
            self.assertFalse([p for p in index[table] if p.endswith(".md")])
        self.assertNotIn("services/demo/src/retired.rs", index["obligations"])
        for table in ("sources", "sourcePrefixes", "conformance", "inputs"):
            self.assertFalse(["x" for ids in index[table].values() if "cantos.publishing.lifecycle" in ids], table)


if __name__ == "__main__":
    unittest.main()
