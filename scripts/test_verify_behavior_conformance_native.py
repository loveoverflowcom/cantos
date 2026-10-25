"""Native Identity auth codec/comparator/registry contract tests, not live implementation evidence."""

import copy
import dataclasses
import json
import re
import tempfile
import tomllib
import unittest
from pathlib import Path
from types import SimpleNamespace
from unittest.mock import patch

import verify_behavior as behavior
import verify_behavior_conformance as conformance
import verify_behavior_conformance_native as native

ROOT = Path(__file__).resolve().parents[1]
UNIT = {"#tup": []}


def to_itf(value, spec):
    """Test-only inverse of the codec, used to build exact ITF fixtures from normalized values."""
    kind = spec[0]
    if kind == "int":
        return {"#bigint": str(value)}
    if kind == "bool":
        return value
    if kind == "enum":
        return {"tag": value, "value": UNIT}
    if kind == "rec":
        return {key: to_itf(value[key], sub) for key, sub in spec[1].items()}
    if kind == "opt":
        return {"tag": spec[1], "value": UNIT} if value is None else {"tag": spec[2], "value": to_itf(value, spec[3])}
    if kind == "sum":
        sub = spec[1][value["tag"]]
        return {"tag": value["tag"], "value": UNIT if sub is None else to_itf(value["value"], sub)}
    if kind == "set":
        return {"#set": [to_itf(item, spec[1]) for item in value]}
    if kind == "map":
        return {"#map": [[to_itf(row[spec[1]], spec[2]), to_itf(row[spec[3]], spec[4])] for row in value]}
    raise AssertionError(kind)


def tag(name, value=None):
    return {"tag": name, "value": value}


def mailbox_state(**changes):
    state = {"durable": {"proof": {"loc": 1, "state": "Pending", "issuedAt": 1, "expiresAt": 3}, "issued": [1], "receipts": [], "accounts": 0},
             "now": 1, "plan": None, "session": False, "studio": False, "admin": False}
    for key, value in changes.items():
        state[key] = value
    return state


def admission_state(**changes):
    state = {"identity": {"read": "Answer", "present": True, "active": True, "epoch": 1, "version": 1}, "rostered": True,
             "session": {"exists": False, "generation": 0, "purpose": "Theatre", "observed": {"subject": 0, "epoch": 0, "version": 0}, "revoked": False},
             "pending": None}
    state.update(changes)
    return state


def raw_trace(specs, steps):
    states = []
    for index, (state, before, event, outcome) in enumerate(steps):
        states.append({"#meta": {"index": index}, "state": to_itf(state, specs["state"]), "before": to_itf(before, specs["state"]),
                       "last": to_itf(event, specs["event"]), "outcome": to_itf(outcome, specs["outcome"])})
    return {"#meta": {"format": "ITF", "source": "source.qnt", "status": "passed"}, "vars": ["state", "before", "last", "outcome"], "states": states}


def mailbox_steps():
    initial = mailbox_state()
    consumed = mailbox_state(durable={"proof": {"loc": 1, "state": "Consumed", "issuedAt": 1, "expiresAt": 3}, "issued": [1],
                                      "receipts": [{"command": 1, "request": REQUEST}], "accounts": 1})
    planned = mailbox_state(plan={"command": 1, "request": REQUEST, "expected": initial["durable"]["proof"],
                                  "next": consumed["durable"]["proof"]})
    return [(initial, initial, tag("NoEvent"), tag("NoOutcome")),
            (initial, initial, tag("Inspect"), tag("NoOp", "Inspected")),
            (planned, initial, tag("Prepare", {"command": 1, "request": REQUEST}), tag("NoOp", "PlanPrepared")),
            (planned, planned, tag("Assertion"), tag("NoOutcome")),
            (consumed, planned, tag("Settle", "Commit"), tag("Transition", "ConsumedWithAccount")),
            (consumed, consumed, tag("Assertion"), tag("NoOutcome"))]


REQUEST = {"operation": 1, "epoch": 1, "intent": "TheatreSignup", "loc": 1, "secret": 1, "explicit": True}
FACT = {"subject": 1, "purpose": "Theatre", "echoed": True, "age": "Fresh", "epoch": 1, "version": 1}


def admission_steps():
    initial = admission_state()
    session = {"exists": True, "generation": 1, "purpose": "Theatre", "observed": {"subject": 1, "epoch": 1, "version": 1}, "revoked": False}
    issued = admission_state(session=session)
    return [(initial, initial, tag("NoEvent"), tag("NoOutcome")),
            (issued, initial, tag("Login", {"purpose": "Theatre", "fact": FACT}), tag("Transition", "Issued")),
            (issued, issued, tag("Assertion"), tag("NoOutcome")),
            (issued, issued, tag("Admit"), tag("NoOp", "Admitted"))]


MAILBOX_CALLS = {"planConfirmation": 0, "planCancellation": 0, "planReplacement": 0}
ADMISSION_CALLS = {"validateFact": 0, "securityStateFromResponse": 0, "decideIssue": 0, "decideAdmission": 0}


def observer_double(case, calls, bump, bindings=False):
    """Echo the expected checkpoints with evidence derived only from each event's tag."""
    totals, rows = dict(calls), []
    for step in case["checkpoints"]:
        decided = "none"
        if step["event"]["tag"] not in ("NoEvent", "Assertion"):
            decided = "harness" if step["event"]["tag"] in bump["inputs"] else "core"
            if decided == "core":
                totals[bump["key"]] += 1
        evidence = {"decidedBy": decided, "calls": dict(totals)}
        if bindings:
            evidence.update({"binding": dict(native.M_BINDING),
                             "planBinding": None if step["state"]["plan"] is None else {"expected": dict(native.M_BINDING), "next": dict(native.M_BINDING)}})
        rows.append({"index": step["index"], "state": copy.deepcopy(step["state"]), "outcome": copy.deepcopy(step["outcome"]), "evidence": evidence})
    return rows


def mailbox_case(identity="stable.mailbox.run"):
    return {"id": identity, "checkpoints": native.decode_trace(raw_trace(native.MAILBOX_SPECS, mailbox_steps()), native.MAILBOX_SPECS)}


def admission_case(identity="stable.admission.run"):
    return {"id": identity, "checkpoints": native.decode_trace(raw_trace(native.ADMISSION_SPECS, admission_steps()), native.ADMISSION_SPECS)}


def mailbox_observation(case):
    rows = observer_double(case, MAILBOX_CALLS, {"inputs": ("Clock",), "key": "planConfirmation"}, bindings=True)
    return {"schemaVersion": 1, "observationSchema": native.MAILBOX_CONTRACT["schema"], "traces": [{"id": case["id"], "checkpoints": rows}]}


def admission_observation(case):
    rows = observer_double(case, ADMISSION_CALLS, {"inputs": ("SetIdentity", "SetRoster"), "key": "decideIssue"})
    return {"schemaVersion": 1, "observationSchema": native.ADMISSION_CONTRACT["schema"], "traces": [{"id": case["id"], "checkpoints": rows}]}


class RegistryContracts(unittest.TestCase):
    def test_the_allowlist_is_finite_and_agrees_with_the_catalog_and_observer_sources(self):
        registry = conformance.adapters()
        self.assertEqual(set(registry), {conformance.CAPABILITY, native.MAILBOX, native.ADMISSION})
        catalog = {cap["id"]: cap for cap in behavior.load_registry(ROOT)[0]}
        registered = {identity for identity, cap in catalog.items() if cap.get("implementationConformance") == "pure-core-observer"}
        self.assertEqual(registered, set(registry), "a catalog observer without an adapter, or the reverse, would be silently skipped")
        for identity, adapter in registry.items():
            self.assertEqual(catalog[identity]["observationSchema"], adapter.schema)
            source = (ROOT / adapter.observer).read_text()
            self.assertIn("fn " + adapter.test.rsplit("::", 1)[1] + "()", source)
            self.assertIn("#[ignore", source, "the replay entry must stay opt-in")
            self.assertIn(adapter.schema, source)
            self.assertTrue(adapter.faults and all(len(fault) == 4 for fault in adapter.faults))

    def test_wrong_capability_schema_or_lifecycle_registration_is_refused(self):
        adapter = native.MAILBOX_ADAPTER
        good = {"id": adapter.capability, "observationSchema": adapter.schema, "implementationConformance": "pure-core-observer"}
        self.assertIs(conformance.adapter_for(good), adapter)
        for edit in (lambda cap: cap.update(id="cantos.identity.native-unregistered"),
                     lambda cap: cap.update(observationSchema=native.ADMISSION_CONTRACT["schema"]),
                     lambda cap: cap.update(observationSchema="cantos-native-mailbox-proof-observation-v2"),
                     lambda cap: cap.pop("observationSchema"),
                     lambda cap: cap.update(implementationConformance="model-only"),
                     lambda cap: cap.pop("implementationConformance")):
            cap = copy.deepcopy(good)
            edit(cap)
            with self.subTest(cap=cap), self.assertRaises(conformance.ConformanceError):
                conformance.adapter_for(cap)

    def test_selecting_every_capability_observes_only_registered_ones_but_an_explicit_choice_must_be_observed(self):
        registered, unregistered = {"implementationConformance": "pure-core-observer"}, {}
        self.assertTrue(behavior.conformance_selected(True, None, registered))
        self.assertFalse(behavior.conformance_selected(True, None, unregistered))
        self.assertTrue(behavior.conformance_selected(True, "any.capability", unregistered), "explicit selection of an unregistered capability must reach the fail-closed adapter lookup")
        self.assertFalse(behavior.conformance_selected(False, None, registered))
        self.assertFalse(behavior.conformance_selected(False, "any.capability", registered))

    def test_release_builds_carry_no_observer_or_test_dependency(self):
        identity = tomllib.loads((ROOT / "services/cantos-identity/Cargo.toml").read_text())
        features = identity["features"]
        for name in ("serde", "serde_json"):
            self.assertIn(name, identity["dev-dependencies"])
            # The executable's Firebase adapter may use JSON; the default/Kani pure core and
            # the persistence shell must never link it.
            if name in identity["dependencies"]:
                self.assertIs(identity["dependencies"][name].get("optional"), True, name)
                self.assertEqual([feature for feature, enabled in features.items() if "dep:" + name in enabled],
                                 ["service"], name)
                self.assertNotIn("service", features.get("default", []) + features["persistence"])
        for relative, parent in (("services/cantos-identity/src/lib.rs", "mod quint_conformance_tests;"),
                                 ("services/cantos-server/src/platform/identity_auth/decision.rs", "mod quint_conformance_tests;")):
            lines = (ROOT / relative).read_text().splitlines()
            index = lines.index(parent)
            self.assertEqual(lines[index - 1], "#[cfg(test)]", relative + " must compile its observer only in tests")
        for adapter in (native.MAILBOX_ADAPTER, native.ADMISSION_ADAPTER):
            source = (ROOT / adapter.observer).read_text()
            self.assertGreaterEqual(len(re.findall(r"^#\[test\]", source, re.MULTILINE)), 5, "the observer keeps its native regression tests")
            self.assertNotIn("pub fn", source, "the observer exposes no production API")
            self.assertNotIn("pub struct", source)


class CodecContracts(unittest.TestCase):
    def test_exact_itf_decodes_to_event_only_replay_input_for_both_capabilities(self):
        for case, tags in ((mailbox_case(), ["NoEvent", "Inspect", "Prepare", "Assertion", "Settle", "Assertion"]),
                           (admission_case(), ["NoEvent", "Login", "Assertion", "Admit"])):
            self.assertEqual([step["event"]["tag"] for step in case["checkpoints"]], tags)
        case = mailbox_case("stable.run.id")
        data = conformance.replay_input([case], native.MAILBOX_CONTRACT["schema"])
        self.assertEqual(data["observationSchema"], native.MAILBOX_CONTRACT["schema"])
        self.assertEqual(set(data["traces"][0]["checkpoints"][2]), {"index", "event"})
        self.assertNotIn("durable", json.dumps(data))
        self.assertEqual(data["traces"][0]["checkpoints"][4]["event"], {"tag": "Settle", "value": "Commit"})
        self.assertEqual(case["checkpoints"][2]["state"]["plan"]["request"], REQUEST)
        self.assertEqual(case["checkpoints"][4]["state"]["durable"]["receipts"], [{"command": 1, "request": REQUEST}])

    def test_malformed_traces_fail_closed_for_both_capabilities(self):
        broken_links = (
            (native.MAILBOX_SPECS, mailbox_steps(), lambda raw: raw["states"][2]["before"]["durable"].update(accounts={"#bigint": "9"})),
            (native.ADMISSION_SPECS, admission_steps(), lambda raw: raw["states"][2]["before"].update(rostered=False)))
        for specs, steps, break_link in broken_links:
            edits = [lambda raw: raw["states"].pop(2),
                     lambda raw: raw["states"].reverse(),
                     lambda raw: raw["states"][2]["#meta"].update(index=1),
                     lambda raw: raw["states"][2].pop("outcome"),
                     lambda raw: raw["states"][1]["last"].update(tag="Upload"),
                     lambda raw: raw["states"][1]["last"].update(value=None),
                     lambda raw: raw["states"][1].update(unregistered=1),
                     lambda raw: raw["states"].clear(),
                     lambda raw: raw.update(vars=["state", "before", "last", "verdict"]),
                     lambda raw: raw["states"][1]["outcome"].update(tag="Granted"),
                     break_link]
            for edit in edits:
                raw = raw_trace(specs, steps)
                edit(raw)
                with self.subTest(edit=edit), self.assertRaises(conformance.ConformanceError):
                    native.decode_trace(raw, specs)

    def test_out_of_domain_classes_and_noncanonical_numbers_fail_closed(self):
        for value in ("+1", "-0", "01", "1.0", "9223372036854775808"):
            with self.subTest(value=value), self.assertRaises(conformance.ConformanceError):
                native.from_itf({"#bigint": value}, native.I(0, native.MAX))
        raw = raw_trace(native.MAILBOX_SPECS, mailbox_steps())
        raw["states"][2]["last"]["value"]["request"]["loc"] = {"#bigint": "4"}
        with self.assertRaises(conformance.ConformanceError):
            native.decode_trace(raw, native.MAILBOX_SPECS)
        raw = raw_trace(native.ADMISSION_SPECS, admission_steps())
        raw["states"][1]["last"]["value"]["fact"]["age"] = {"tag": "Ancient", "value": UNIT}
        with self.assertRaises(conformance.ConformanceError):
            native.decode_trace(raw, native.ADMISSION_SPECS)
        raw = raw_trace(native.MAILBOX_SPECS, mailbox_steps())
        raw["states"][1]["state"]["durable"]["issued"]["#set"].append({"#bigint": "1"})
        raw["states"][2]["before"]["durable"]["issued"]["#set"].append({"#bigint": "1"})
        with self.assertRaises(conformance.ConformanceError):
            native.decode_trace(raw, native.MAILBOX_SPECS)

    def test_effectful_assertion_is_never_a_positive_trace(self):
        cases = (
            (native.MAILBOX_SPECS, mailbox_steps(), 3,
             lambda state: {**state, "durable": {**state["durable"], "accounts": 7}}),
            (native.ADMISSION_SPECS, admission_steps(), 2, lambda state: {**state, "rostered": False}))
        for specs, steps, index, mutate in cases:
            for part in ("outcome", "state"):
                bad = list(steps)
                state, before, event, outcome = bad[index]
                bad[index] = (state, before, event, tag("External")) if part == "outcome" else (mutate(state), before, event, outcome)
                with self.subTest(index=index, part=part), self.assertRaises(conformance.ConformanceError):
                    native.decode_trace(raw_trace(specs, bad), specs)


class ComparatorContracts(unittest.TestCase):
    def contract(self, which):
        return (native.MAILBOX_ADAPTER, mailbox_case, mailbox_observation) if which == "mailbox" else (native.ADMISSION_ADAPTER, admission_case, admission_observation)

    def test_faithful_observations_match_and_first_semantic_mismatch_is_recorded(self):
        for which in ("mailbox", "admission"):
            adapter, make, observe = self.contract(which)
            case = make()
            self.assertEqual(adapter.compare([case], observe(case)), [], which)
            observed = observe(case)
            index = 4 if which == "mailbox" else 1
            observed["traces"][0]["checkpoints"][index]["outcome"] = tag("External")
            row = adapter.compare([case], observed)[0]
            self.assertEqual((row["index"], row["fields"]), (index, ["outcome.tag", "outcome.value"] if which == "admission" else ["outcome.tag", "outcome.value"]), which)
            self.assertEqual(row["event"], case["checkpoints"][index]["event"])
            observed = observe(case)
            state = observed["traces"][0]["checkpoints"][index]["state"]
            state["now" if which == "mailbox" else "rostered"] = 5 if which == "mailbox" else False
            row = adapter.compare([case], observed)[0]
            self.assertEqual((row["index"], row["fields"]), (index, ["state.now" if which == "mailbox" else "state.rostered"]))

    def test_a_deliberately_wrong_model_mismatches_semantically_while_parse_failures_never_do(self):
        adapter, make, observe = self.contract("mailbox")
        expected, observed = make(), observe(make())
        mutant = copy.deepcopy(expected)
        mutant["checkpoints"][4]["state"]["durable"]["accounts"] = 2
        row = adapter.compare([mutant], observed)[0]
        self.assertEqual((row["index"], row["fields"]), (4, ["state.durable.accounts"]))
        for garbage in (lambda data: data["traces"][0]["checkpoints"][4].pop("evidence"),
                        lambda data: data["traces"][0]["checkpoints"][4]["state"].update(extra=1),
                        lambda data: data["traces"][0]["checkpoints"][4]["outcome"].update(tag="Unknown"),
                        lambda data: data["traces"][0]["checkpoints"].pop()):
            damaged = copy.deepcopy(observed)
            garbage(damaged)
            with self.subTest(garbage=garbage), self.assertRaises(conformance.ConformanceError):
                adapter.compare([mutant], damaged)

    def test_missing_reordered_schema_or_cross_capability_observations_reject(self):
        for which in ("mailbox", "admission"):
            adapter, make, observe = self.contract(which)
            case = make()
            edits = [lambda data: data.update(observationSchema="v2"),
                     lambda data: data.update(observationSchema=(native.ADMISSION_CONTRACT if which == "mailbox" else native.MAILBOX_CONTRACT)["schema"]),
                     lambda data: data["traces"].clear(),
                     lambda data: data["traces"][0].update(id="other"),
                     lambda data: data["traces"][0]["checkpoints"].pop(),
                     lambda data: data["traces"][0]["checkpoints"][2].update(index=1),
                     lambda data: data["traces"][0]["checkpoints"].reverse(),
                     lambda data: data.update(schemaVersion=2)]
            for edit in edits:
                observed = observe(case)
                edit(observed)
                with self.subTest(which=which, edit=edit), self.assertRaises(conformance.ConformanceError):
                    adapter.compare([case], observed)

    def test_origin_labels_must_match_production_call_counters(self):
        adapter, make, observe = self.contract("mailbox")
        case = make()
        calls = lambda data, index: data["traces"][0]["checkpoints"][index]["evidence"]
        edits = [lambda data: calls(data, 3).update(decidedBy="core"),                           # assertion claims a decision
                 lambda data: calls(data, 3)["calls"].update(planConfirmation=9),               # assertion repeated a production call
                 lambda data: calls(data, 2).update(decidedBy="shell"),                          # shell label with a production call
                 lambda data: calls(data, 2).update(decidedBy="none"),                           # decision event with no origin
                 lambda data: calls(data, 4)["calls"].update(planConfirmation=0),               # counters moved backwards
                 lambda data: calls(data, 1).update(decidedBy="mixed")]
        for edit in edits:
            observed = observe(case)
            edit(observed)
            with self.subTest(edit=edit), self.assertRaises(conformance.ConformanceError):
                adapter.compare([case], observed)

    def test_mailbox_binding_and_plan_binding_are_pinned_to_the_fixture(self):
        adapter, make, observe = self.contract("mailbox")
        case = make()
        for edit in (lambda data: data["traces"][0]["checkpoints"][2]["evidence"]["binding"].update(intent="TheatreRecovery"),
                     lambda data: data["traces"][0]["checkpoints"][2]["evidence"]["binding"].update(epoch=2),
                     lambda data: data["traces"][0]["checkpoints"][2]["evidence"]["planBinding"]["next"].update(operation=2),
                     lambda data: data["traces"][0]["checkpoints"][2]["evidence"].update(planBinding=None),
                     lambda data: data["traces"][0]["checkpoints"][1]["evidence"].update(planBinding={"expected": dict(native.M_BINDING), "next": dict(native.M_BINDING)})):
            observed = observe(case)
            edit(observed)
            with self.subTest(edit=edit), self.assertRaises(conformance.ConformanceError):
                adapter.compare([case], observed)

    def test_boolean_and_integer_observations_never_alias(self):
        adapter, make, observe = self.contract("mailbox")
        case = make()
        for edit in (lambda data: data["traces"][0]["checkpoints"][4]["state"]["durable"].update(accounts=True),
                     lambda data: data["traces"][0]["checkpoints"][4]["state"].update(session=0),
                     lambda data: data["traces"][0]["checkpoints"][4]["state"]["durable"]["proof"].update(loc=True)):
            observed = observe(case)
            edit(observed)
            with self.subTest(edit=edit), self.assertRaises(conformance.ConformanceError):
                adapter.compare([case], observed)
        adapter, make, observe = self.contract("admission")
        case = make()
        observed = observe(case)
        observed["traces"][0]["checkpoints"][3]["state"]["session"]["generation"] = True
        with self.assertRaises(conformance.ConformanceError):
            adapter.compare([case], observed)

    def test_observer_fault_is_a_semantic_mismatch_on_the_original_trace(self):
        adapter, make, observe = self.contract("admission")
        case = make()
        observed = observe(case)
        observed["traces"][0]["checkpoints"][3]["outcome"] = tag("Rejected", "SecurityChanged")
        observed["traces"][0]["checkpoints"][3]["state"]["pending"] = "Disabled"
        row = adapter.compare([case], observed)[0]
        self.assertEqual(row["index"], 3)
        self.assertIn("state.pending", row["fields"])

    def test_summary_counts_origins_and_production_calls_for_nonvacuity(self):
        adapter, make, observe = self.contract("mailbox")
        summary = adapter.summary(observe(make()))
        self.assertEqual(summary["productionCalls"], {"planConfirmation": 3, "planCancellation": 0, "planReplacement": 0})
        self.assertEqual(summary["checkpointsByOrigin"]["none"], 3)
        self.assertEqual(sum(summary["checkpointsByOrigin"].values()), 6)


class DetectionReasonContracts(unittest.TestCase):
    def row(self, name, event, outcome, fields, origin="core"):
        return {"id": "cantos.identity.native-mailbox-proof.control." + name, "event": {"tag": event, "value": None},
                "observed": {"outcome": {"tag": outcome[0], "value": outcome[1]}}, "fields": fields, "origin": origin}

    def test_registered_reasons_cover_exactly_the_catalog_controls(self):
        catalog = {cap["id"]: cap for cap in behavior.load_registry(ROOT)[0]}
        for adapter in (native.MAILBOX_ADAPTER, native.ADMISSION_ADAPTER):
            names = {row["id"].rsplit(".control.", 1)[-1] for row in catalog[adapter.capability]["controls"]}
            self.assertEqual(set(adapter.reasons), names)

    def test_a_control_counts_only_when_its_removed_rule_fires_on_the_correct_core(self):
        adapter = native.MAILBOX_ADAPTER
        self.assertEqual(conformance.check_reason(adapter, self.row("explicit-only", "Prepare", ("Rejected", "NotExplicit"), ["outcome.value", "state.plan"])), "core")
        self.assertEqual(conformance.check_reason(adapter, self.row("no-authority", "Settle", ("UnknownEffect", None), ["state.session"], "core+shell")), "harness")
        self.assertEqual(conformance.check_reason(adapter, self.row("plan-has-no-effect", "Prepare", ("NoOp", "PlanPrepared"), ["state.durable.proof.state"])), "shell")
        for wrong in (self.row("explicit-only", "Settle", ("Rejected", "NotExplicit"), ["outcome.value"]),
                      self.row("explicit-only", "Prepare", ("Rejected", "Expired"), ["outcome.value"]),
                      self.row("no-authority", "Settle", ("UnknownEffect", None), ["state.durable.accounts"]),
                      self.row("plan-has-no-effect", "Prepare", ("NoOp", "PlanPrepared"), ["state.plan"]),
                      self.row("unregistered-control", "Prepare", ("Rejected", "NotExplicit"), [])):
            with self.subTest(row=wrong["id"]), self.assertRaises(conformance.ConformanceError):
                conformance.check_reason(adapter, wrong)

    def test_a_core_kill_needs_a_differing_outcome_at_a_core_checkpoint(self):
        adapter = native.MAILBOX_ADAPTER
        for origin, fields in (("shell", ["outcome.value"]), ("core+shell", ["outcome.value"]), ("core", ["state.plan"])):
            with self.subTest(origin=origin, fields=fields), self.assertRaisesRegex(conformance.ConformanceError, "wrong reason"):
                conformance.check_reason(adapter, self.row("explicit-only", "Prepare", ("Rejected", "NotExplicit"), fields, origin))
        unlabelled = dataclasses.replace(adapter, reasons={"explicit-only": {**native.reason("Prepare", [("Rejected", "NotExplicit")]), "by": None}})
        with self.assertRaisesRegex(conformance.ConformanceError, "no registered detector"):
            conformance.check_reason(unlabelled, self.row("explicit-only", "Prepare", ("Rejected", "NotExplicit"), ["outcome.value"]))

    def test_shell_and_harness_detections_are_declared_apart_from_core(self):
        detectors = {adapter.capability: {name: row["by"] for name, row in adapter.reasons.items()}
                     for adapter in (native.MAILBOX_ADAPTER, native.ADMISSION_ADAPTER)}
        self.assertEqual({name for name, by in detectors[native.MAILBOX].items() if by != "core"},
                         {"plan-has-no-effect", "commit-compares-snapshot", "no-authority", "only-committed-effects"})
        self.assertEqual({name for name, by in detectors[native.ADMISSION].items() if by != "core"},
                         {"request-is-not-commit", "revocation-sticky", "session-changes-only-by-transition"})
        self.assertEqual(detectors[native.MAILBOX]["no-authority"], "harness")


class NativeIdentityFixture(unittest.TestCase):
    """Selection/identity checks for a native capability, mirroring the audio fixture."""

    def fixture(self):
        directory = tempfile.TemporaryDirectory(prefix="cantos-native-conformance-contract-")
        self.addCleanup(directory.cleanup)
        root, bundle = Path(directory.name), Path(directory.name) / "bundle"
        bundle.mkdir()
        adapter = native.MAILBOX_ADAPTER
        cap = {"id": adapter.capability, "observationSchema": adapter.schema, "implementationConformance": "pure-core-observer",
               "model": "model.qnt", "scenarios": "scenarios.qnt", "simulation": {"scenarioSamples": 1, "seed": "334"},
               "properties": [{"id": "property", "declaration": "prop"}], "runs": [{"id": "run", "declaration": "test"}],
               "witnesses": [{"id": "witness", "declaration": "witness"}],
               "controls": [{"id": "control", "property": "property", "from": "original", "to": "mutated"}]}
        inputs = {"model.qnt": "original", "scenarios.qnt": "named"}
        for path, data in inputs.items():
            (root / path).write_text(data)
        inputs = {path: behavior.digest(root / path) for path in inputs}
        behavior.retain_inputs(root, bundle, inputs)
        (bundle / "installation.json").write_text("{}")
        versions = {"installationReceiptSha256": behavior.digest(bundle / "installation.json")}
        report = {"schemaVersion": 1, "scope": "model-only", "status": "PASS", "capability": cap["id"], "sourceRevision": "revision",
                  "inputs": inputs, "tools": versions, "budget": cap["simulation"],
                  "selection": {key: cap[key] for key in ("properties", "runs", "witnesses")},
                  "controlSelection": [{"id": "control", "property": "property"}],
                  "observationVariables": sorted(adapter.variables),
                  "stages": {key: "PASS" for key in ("typecheck", "scenarios", "simulation", "witnesses", "negativeControls", "freshness")},
                  "installationReceipt": {"file": "installation.json", "sha256": versions["installationReceiptSha256"]}, "traces": {}}
        (bundle / "control-1.qnt").write_text("mutated")
        for key, source, status in (("test", root / "scenarios.qnt", "passed"), ("properties", root / "model.qnt", "ok"),
                                    ("witness-witness", root / "model.qnt", "violation"), ("control-1", bundle / "control-1.qnt", "violation")):
            path = bundle / (key + ".itf.json")
            raw = raw_trace(native.MAILBOX_SPECS, mailbox_steps())
            raw["#meta"].update(source=str(source), status=status)
            behavior.write_json(path, raw)
            item = {"file": path.name, "sha256": behavior.digest(path)}
            report["traces"][key] = [item] if key == "test" else item
        return root, cap, bundle, report, inputs, versions

    def test_fresh_native_identity_selects_every_trace_and_stale_or_partial_evidence_fails(self):
        root, cap, bundle, report, inputs, versions = self.fixture()
        cases = conformance.collect_cases(root, cap, bundle, report, inputs, versions, "revision", behavior)
        self.assertEqual([case["sensitivity"] for case in cases], [False, False, False, True])
        edits = [lambda cap, report: cap.update(observationSchema=native.ADMISSION_CONTRACT["schema"]),
                 lambda cap, report: cap.update(id=native.ADMISSION),
                 lambda cap, report: report.update(observationVariables=["state", "before", "last", "verdict"]),
                 lambda cap, report: report["stages"].update(witnesses="NOT_RUN"),
                 lambda cap, report: report["traces"].update(test=[]),
                 lambda cap, report: report["traces"].pop("witness-witness"),
                 lambda cap, report: report["traces"].pop("control-1"),
                 lambda cap, report: report["traces"]["properties"].update(sha256="stale"),
                 lambda cap, report: report.update(sourceRevision="stale")]
        for edit in edits:
            current_cap, current_report = copy.deepcopy(cap), copy.deepcopy(report)
            edit(current_cap, current_report)
            with self.subTest(edit=edit), self.assertRaises(conformance.ConformanceError):
                conformance.collect_cases(root, current_cap, bundle, current_report, inputs, versions, "revision", behavior)

    def test_a_decoded_audio_trace_is_not_accepted_for_a_native_capability(self):
        root, cap, bundle, report, inputs, versions = self.fixture()
        path = bundle / "properties.itf.json"
        raw = json.loads(path.read_text())
        raw["states"][1]["last"] = {"tag": "Begin", "value": {"digest": {"#bigint": "0"}, "length": {"#bigint": "0"}}}
        behavior.write_json(path, raw)
        report["traces"]["properties"]["sha256"] = behavior.digest(path)
        with self.assertRaises(conformance.ConformanceError):
            conformance.collect_cases(root, cap, bundle, report, inputs, versions, "revision", behavior)

    def test_zero_or_skipped_observer_tests_fail_the_native_stage_and_keep_the_partial_report(self):
        for stdout in ("test result: ok. 0 passed; 0 failed; 0 ignored;", "test result: ok. 1 passed; 0 failed; 1 ignored;", "no summary"):
            root, cap, bundle, report, inputs, versions = self.fixture()
            behavior.write_json(bundle / "report.json", report)
            case = {"id": "run", "checkpoints": mailbox_case()["checkpoints"], "sensitivity": False,
                    "source": "source", "file": "file", "sha256": "hash", "seed": "334"}
            tools = {"cargo": {"toolchainBinary": "cargo"}, "rustc": {"toolchainBinary": "rustc"}}
            with patch.object(conformance, "snapshot", return_value={}), \
                 patch.object(conformance, "compiler_configuration", return_value={}), \
                 patch.object(conformance, "implementation_tools", return_value=tools), \
                 patch.object(conformance, "collect_cases", return_value=[case]), \
                 patch.object(conformance, "observer_environment", return_value={}), \
                 patch.object(conformance, "bounded_run", return_value=SimpleNamespace(returncode=0, stdout=stdout)), \
                 self.subTest(stdout=stdout), self.assertRaises(conformance.ConformanceError):
                conformance.execute(root, cap, bundle, report, versions, inputs, "revision", behavior)
            partial = behavior.read_json(bundle / "conformance.json")
            self.assertEqual((partial["status"], partial["stages"]["originalTraces"], partial["stages"]["mutantSensitivity"]), ("FAIL", "FAIL", "NOT_RUN"))
            self.assertEqual(partial["capability"], native.MAILBOX)
            self.assertEqual(behavior.read_json(bundle / "report.json")["status"], "PASS", "model and conformance reports stay independent")

    def test_no_positive_trace_selection_fails_before_any_observer_runs(self):
        root, cap, bundle, report, inputs, versions = self.fixture()
        behavior.write_json(bundle / "report.json", report)
        sensitivity = {"id": "control", "checkpoints": mailbox_case()["checkpoints"], "sensitivity": True,
                       "source": "source", "file": "file", "sha256": "hash", "seed": "334"}
        tools = {"cargo": {"toolchainBinary": "cargo"}, "rustc": {"toolchainBinary": "rustc"}}
        with patch.object(conformance, "snapshot", return_value={}), \
             patch.object(conformance, "compiler_configuration", return_value={}), \
             patch.object(conformance, "implementation_tools", return_value=tools), \
             patch.object(conformance, "collect_cases", return_value=[sensitivity]), \
             patch.object(conformance, "bounded_run") as run, self.assertRaises(conformance.ConformanceError):
            conformance.execute(root, cap, bundle, report, versions, inputs, "revision", behavior)
        run.assert_not_called()
        self.assertEqual(behavior.read_json(bundle / "conformance.json")["stages"]["originalTraces"], "FAIL")


class StageLogicContracts(unittest.TestCase):
    """The stage machine with a controllable observer double, never a live implementation."""

    COUNTER = {"Prepare": "planConfirmation", "Inspect": "planConfirmation", "Settle": "planConfirmation",
               "Cancel": "planCancellation", "Replace": "planReplacement"}

    def truth(self):
        initial = mailbox_state()
        cancelled = mailbox_state(durable={**initial["durable"], "proof": {**initial["durable"]["proof"], "state": "Cancelled"}})
        steps = [(initial, initial, tag("NoEvent"), tag("NoOutcome")),
                 (initial, initial, tag("Prepare", {"command": 1, "request": {**REQUEST, "explicit": False}}), tag("Rejected", "NotExplicit")),
                 (cancelled, initial, tag("Cancel", 1), tag("Transition", "CancelledProof")),
                 (cancelled, cancelled, tag("Replace", {"loc": 1, "fresh": 2}), tag("Rejected", "AlreadyCancelled")),
                 (cancelled, cancelled, tag("Settle", "Commit"), tag("Rejected", "NothingPlanned"))]
        return native.decode_trace(raw_trace(native.MAILBOX_SPECS, steps), native.MAILBOX_SPECS)

    def observer(self, truth, detects_faults=True, crash_on=None):
        def run(command, cwd, env, timeout, log):
            data = json.loads(Path(env["CANTOS_QUINT_REPLAY_INPUT"]).read_text())
            Path(log).write_text("observer log")
            if crash_on and crash_on in data["traces"][0]["id"] and len(data["traces"][0]["checkpoints"]) < 3:
                return SimpleNamespace(returncode=101, stdout="thread panicked")
            traces = []
            for trace in data["traces"]:
                steps, totals, rows = truth[: len(trace["checkpoints"])], dict.fromkeys(("planConfirmation", "planCancellation", "planReplacement"), 0), []
                for step in steps:
                    key = self.COUNTER.get(step["event"]["tag"])
                    if key:
                        totals[key] += 1
                    decided = "none" if step["event"]["tag"] in ("NoEvent", "Assertion") else "core"
                    binding = dict(native.M_BINDING)
                    rows.append({"index": step["index"], "state": copy.deepcopy(step["state"]), "outcome": copy.deepcopy(step["outcome"]),
                                 "evidence": {"decidedBy": decided, "calls": dict(totals), "binding": binding, "planBinding": None}})
                if env.get("CANTOS_QUINT_OBSERVER_FAULT") and detects_faults:
                    rows[1]["outcome"] = tag("NoOp", "PlanPrepared")
                traces.append({"id": trace["id"], "checkpoints": rows})
            Path(env["CANTOS_QUINT_REPLAY_OUTPUT"]).write_text(json.dumps({"schemaVersion": 1, "observationSchema": native.MAILBOX_CONTRACT["schema"], "traces": traces}))
            return SimpleNamespace(returncode=0, stdout="test result: ok. 1 passed; 0 failed; 0 ignored;")
        return run

    def stage(self, detects_faults=True, mutant=None, crash_on=None, reasons=None):
        directory = tempfile.TemporaryDirectory(prefix="cantos-native-stage-contract-")
        self.addCleanup(directory.cleanup)
        root, bundle = Path(directory.name), Path(directory.name) / "bundle"
        bundle.mkdir()
        identity = native.MAILBOX
        adapter = dataclasses.replace(
            native.MAILBOX_ADAPTER, reasons=reasons if reasons is not None else {"explicit-only": native.reason("Prepare", [("Rejected", "NotExplicit")])},
            faults=(("fault-inspect-as-explicit", "inspect-as-explicit", ("run.explicit-confirmation",), "fault escaped"),))
        cap = {"id": identity, "observationSchema": adapter.schema, "implementationConformance": "pure-core-observer",
               "controls": [{"id": identity + ".control.explicit-only", "property": identity + ".property.explicit-only"}]}
        truth = self.truth()
        wrong = copy.deepcopy(truth)
        wrong[1]["outcome"], wrong[1]["state"] = tag("NoOp", "PlanPrepared"), mailbox_state(plan={"command": 1, "request": REQUEST, "expected": truth[0]["state"]["durable"]["proof"], "next": truth[0]["state"]["durable"]["proof"]})
        if mutant is not None:
            wrong = mutant(wrong)
        cases = [{"id": identity + ".run.explicit-confirmation", "checkpoints": truth, "sensitivity": False, "source": "s", "file": "f", "sha256": "h", "seed": "334"},
                 {"id": identity + ".control.explicit-only", "checkpoints": wrong, "sensitivity": True, "source": "s", "file": "f", "sha256": "h", "seed": "334"}]
        behavior.write_json(bundle / "report.json", {"status": "PASS"})
        tools = {"cargo": {"toolchainBinary": "cargo"}, "rustc": {"toolchainBinary": "rustc"}}
        with patch.object(conformance, "adapters", return_value={identity: adapter}), \
             patch.object(conformance, "snapshot", return_value={}), \
             patch.object(conformance, "compiler_configuration", return_value={}), \
             patch.object(conformance, "implementation_tools", return_value=tools), \
             patch.object(conformance, "collect_cases", return_value=cases), \
             patch.object(conformance, "bounded_run", side_effect=self.observer(truth, detects_faults, crash_on)), \
             patch.object(behavior, "snapshot", return_value={}), patch.object(behavior, "installed_tool", return_value=({}, None, None)), \
             patch.object(behavior, "process", return_value=SimpleNamespace(returncode=0, stdout="revision\n")):
            try:
                conformance.execute(root, cap, bundle, {}, {}, {}, "revision", behavior)
            finally:
                result = behavior.read_json(bundle / "conformance.json")
        return result

    def test_original_mutant_and_observer_fault_stages_pass_with_a_semantic_kill(self):
        result = self.stage()
        self.assertEqual(result["status"], "PASS")
        self.assertEqual(set(result["stages"].values()), {"PASS"})
        self.assertEqual([(row["index"], row["event"]["tag"]) for row in result["mutantMismatches"]], [(1, "Prepare")])
        self.assertEqual(result["mutantKillsByDetector"], {"core": 1, "shell": 0, "harness": 0})
        self.assertEqual(result["observerFaults"][0]["detected"], [native.MAILBOX + ".run.explicit-confirmation"])
        self.assertEqual(result["observationSummary"]["productionCalls"], {"planConfirmation": 2, "planCancellation": 1, "planReplacement": 1})
        self.assertEqual(result["scope"], "pure-core-synthetic-shell")
        self.assertIn("MailboxProof::plan_confirmation", result["productionSymbols"])
        self.assertTrue(result["syntheticShell"])

    def test_an_undetected_observer_fault_fails_the_sensitivity_stage(self):
        with self.assertRaisesRegex(conformance.ConformanceError, "fault escaped"):
            self.stage(detects_faults=False)

    def test_a_crashing_observer_is_not_a_killed_mutant(self):
        with self.assertRaisesRegex(conformance.ConformanceError, "observer failed or selected zero/incorrect tests"):
            self.stage(crash_on="control")

    def test_a_model_control_the_correct_core_does_not_distinguish_is_a_failure(self):
        same = lambda wrong: copy.deepcopy(self.truth())
        with self.assertRaisesRegex(conformance.ConformanceError, "did not distinguish a deliberately wrong model control"):
            self.stage(mutant=same)

    def test_a_kill_for_the_wrong_reason_or_without_a_registered_reason_is_a_failure(self):
        for reasons, message in (({"explicit-only": native.reason("Settle", [("Rejected", "NotExplicit")])}, "wrong reason"),
                                 ({"explicit-only": native.reason("Prepare", [("Rejected", "Expired")])}, "wrong reason"),
                                 ({"other-control": native.reason("Prepare", [("Rejected", "NotExplicit")])}, "do not match the catalog controls")):
            with self.subTest(reasons=reasons), self.assertRaisesRegex(conformance.ConformanceError, message):
                self.stage(reasons=reasons)


if __name__ == "__main__":
    unittest.main()
