"""Native Identity auth (#334/#335) ITF codecs and exact comparators, declared as data.

Each capability declares one spec per observation variable. The same spec decodes the pinned
Quint ITF representation (the expected side, which never reaches Rust) and strictly validates
the Rust observation (the actual side). Nothing here evaluates a transition, derives an
expected outcome or reproduces a production decision: expectations are only decoded.

Scope is `pure-core-synthetic-shell`. Every observed checkpoint states whether a production
seam, the synthetic shell or only a harness input produced it, so a report cannot present a
synthetic shell decision as production evidence.
"""

from verify_behavior_conformance import (Adapter, ConformanceError, boolean, differing, integer, record, require,
                                         unit, variant)

MAX = 2**63 - 1
MAILBOX = "cantos.identity.native-mailbox-proof"
ADMISSION = "cantos.identity.native-session-admission"
ORIGINS = ("none", "core", "shell", "core+shell", "harness")


# Spec constructors. A spec is a plain tuple; no behavior lives in a spec.
def I(low, high):
    return ("int", low, high)


B = ("bool",)


def E(*tags):
    return ("enum", frozenset(tags))


def R(fields):
    return ("rec", fields)


def OPT(none, some, spec):
    return ("opt", none, some, spec)


def SUM(tags):
    return ("sum", tags)


def SET(spec):
    return ("set", spec)


def MAP(key, key_spec, value, value_spec):
    return ("map", key, key_spec, value, value_spec)


def from_itf(value, spec):
    """Decode the exact Quint ITF representation into the observation's normalized JSON form."""
    kind = spec[0]
    if kind == "int":
        return integer(value, spec[1], spec[2])
    if kind == "bool":
        return boolean(value)
    if kind == "enum":
        tag, payload = variant(value, spec[1])
        unit(payload)
        return tag
    if kind == "rec":
        record(value, list(spec[1]), "ITF record")
        return {key: from_itf(value[key], sub) for key, sub in spec[1].items()}
    if kind == "opt":
        tag, payload = variant(value, {spec[1], spec[2]})
        if tag == spec[1]:
            unit(payload)
            return None
        return from_itf(payload, spec[3])
    if kind == "sum":
        tag, payload = variant(value, set(spec[1]))
        if spec[1][tag] is None:
            unit(payload)
            return {"tag": tag, "value": None}
        return {"tag": tag, "value": from_itf(payload, spec[1][tag])}
    if kind == "set":
        record(value, ["#set"], "ITF set")
        require(isinstance(value["#set"], list), "invalid ITF set")
        items = [from_itf(item, spec[1]) for item in value["#set"]]
        require(len(set(items)) == len(items), "duplicate ITF set member")
        return sorted(items)
    if kind == "map":
        record(value, ["#map"], "ITF map")
        require(isinstance(value["#map"], list) and all(isinstance(pair, list) and len(pair) == 2 for pair in value["#map"]), "invalid ITF map")
        rows = [{spec[1]: from_itf(key, spec[2]), spec[3]: from_itf(item, spec[4])} for key, item in value["#map"]]
        require(len({row[spec[1]] for row in rows}) == len(rows), "duplicate ITF map key")
        return sorted(rows, key=lambda row: row[spec[1]])
    raise ConformanceError("unknown observation spec")


def check(value, spec, context="observation"):
    """Strictly validate one observed JSON value; Boolean and integer never alias."""
    kind = spec[0]
    if kind == "int":
        require(type(value) is int and spec[1] <= value <= spec[2], "invalid " + context + " integer")
    elif kind == "bool":
        require(type(value) is bool, "invalid " + context + " Boolean")
    elif kind == "enum":
        require(isinstance(value, str) and value in spec[1], "unknown " + context + " variant")
    elif kind == "rec":
        record(value, list(spec[1]), context)
        for key, sub in spec[1].items():
            check(value[key], sub, context + "." + key)
    elif kind == "opt":
        if value is not None:
            check(value, spec[3], context)
    elif kind == "sum":
        record(value, ["tag", "value"], context)
        require(isinstance(value["tag"], str) and value["tag"] in spec[1], "unknown " + context + " variant")
        if spec[1][value["tag"]] is None:
            require(value["value"] is None, "invalid " + context + " payload")
        else:
            check(value["value"], spec[1][value["tag"]], context + ".value")
    elif kind == "set":
        require(isinstance(value, list), "invalid " + context + " set")
        for item in value:
            check(item, spec[1], context)
        require(value == sorted(set(value)), "unordered or duplicate " + context + " set")
    elif kind == "map":
        require(isinstance(value, list), "invalid " + context + " map")
        for row in value:
            check(row, R({spec[1]: spec[2], spec[3]: spec[4]}), context)
        keys = [row[spec[1]] for row in value]
        require(keys == sorted(set(keys)), "unordered or duplicate " + context + " map")
    else:
        raise ConformanceError("unknown observation spec")


def decode_trace(raw, specs):
    """Decode a complete ITF trace; every checkpoint, event and before-link must be present."""
    require(isinstance(raw, dict) and raw.get("vars") == ["state", "before", "last", "outcome"], "unsupported ITF variable representation")
    checkpoints = raw.get("states")
    require(isinstance(checkpoints, list) and 2 <= len(checkpoints) <= 101, "empty or oversized ITF trace")
    result, previous = [], None
    for index, checkpoint in enumerate(checkpoints):
        record(checkpoint, ["#meta", "state", "before", "last", "outcome"], "ITF checkpoint")
        meta = record(checkpoint["#meta"], ["index"], "ITF checkpoint metadata")
        require(type(meta["index"]) is int and meta["index"] == index, "missing or reordered ITF checkpoint")
        state, before = from_itf(checkpoint["state"], specs["state"]), from_itf(checkpoint["before"], specs["state"])
        action, outcome = from_itf(checkpoint["last"], specs["event"]), from_itf(checkpoint["outcome"], specs["outcome"])
        require((action["tag"] == "NoEvent") == (index == 0), "NoEvent only valid at initial checkpoint")
        require(before == (state if index == 0 else previous), "ITF before checkpoint is not prior state")
        if action["tag"] in ("NoEvent", "Assertion"):
            require(outcome == {"tag": "NoOutcome", "value": None}, "effectful assertion/initial outcome")
        if action["tag"] == "Assertion":
            require(state == before, "effectful assertion checkpoint")
        result.append({"index": index, "event": action, "state": state, "outcome": outcome})
        previous = state
    return result


def check_evidence(adapter, event, evidence, previous):
    """`shell` versus a production origin must agree with the actual call counters.

    Whether a production checkpoint is `core` or `core+shell` remains the observer's declaration.
    """
    keys = list(adapter["calls"])
    before = previous["calls"] if previous else {key: 0 for key in keys}
    require(all(evidence["calls"][key] >= before[key] for key in keys), "production call counters decreased")
    delta = sum(evidence["calls"][key] - before[key] for key in keys)
    decided, tag = evidence["decidedBy"], event["tag"]
    if tag in ("NoEvent", "Assertion"):
        require(decided == "none" and delta == 0, "assertion or initial checkpoint repeated a decision")
    elif tag in adapter["inputs"]:
        require(decided == "harness" and delta == 0, "input fact checkpoint called a production decision")
    else:
        require(decided in ("core", "shell", "core+shell"), "decision event without a decision origin")
        require((delta > 0) == (decided != "shell"), "decision origin disagrees with production call counters")


def compare(adapter, cases, observed):
    record(observed, ["schemaVersion", "observationSchema", "traces"], "observer envelope")
    require(observed["schemaVersion"] == 1 and observed["observationSchema"] == adapter["schema"], "unsupported observer schema")
    rows = observed["traces"]
    require(isinstance(rows, list) and all(isinstance(row, dict) for row in rows) and
            [row.get("id") for row in rows] == [case["id"] for case in cases], "missing/reordered observer traces")
    mismatches = []
    for case, row in zip(cases, rows):
        record(row, ["id", "checkpoints"], "observer trace")
        checkpoints = row["checkpoints"]
        require(isinstance(checkpoints, list) and len(checkpoints) == len(case["checkpoints"]), "missing observer checkpoints")
        previous, first = None, None
        for expected, actual in zip(case["checkpoints"], checkpoints):
            check(actual, R({"index": I(0, 100), "state": adapter["state"], "outcome": adapter["outcome"], "evidence": adapter["evidence"]}))
            require(actual["index"] == expected["index"], "reordered observer checkpoints")
            check_evidence(adapter, expected["event"], actual["evidence"], previous)
            adapter["extra"](expected, actual)
            if first is None and (actual["state"] != expected["state"] or actual["outcome"] != expected["outcome"]):
                first = {"id": case["id"], "index": expected["index"], "event": expected["event"],
                         "expected": {key: expected[key] for key in ("state", "outcome")},
                         "observed": {key: actual[key] for key in ("state", "outcome")},
                         "origin": actual["evidence"]["decidedBy"]}
                first["fields"] = differing(first["expected"], first["observed"])
            previous = actual["evidence"]
        if first is not None:
            mismatches.append(first)
    return mismatches


def summarize(adapter, observed):
    origins = {origin: 0 for origin in ORIGINS}
    calls = {key: 0 for key in adapter["calls"]}
    for trace in observed["traces"]:
        for checkpoint in trace["checkpoints"]:
            origins[checkpoint["evidence"]["decidedBy"]] += 1
        for key in calls:
            calls[key] += trace["checkpoints"][-1]["evidence"]["calls"][key]
    return {"checkpointsByOrigin": origins, "productionCalls": calls}


# ---- cantos.identity.native-mailbox-proof -----------------------------------------------------
INTENTS = ("TheatreSignup", "TheatreRecovery", "StudioRecovery")
MAILBOX_REASONS = ("EpochChanged", "OperationReused", "NotExplicit", "UnknownOperation", "BindingChanged", "ProofChanged",
                   "InvalidSecret", "AlreadyConsumed", "AlreadyCancelled", "BeforeIssue", "Expired", "NothingPlanned",
                   "StaleSnapshot", "CommitRejected", "ReusedLocator", "LocatorNotUnique", "Cooldown")
M_REQUEST = R({"operation": I(1, 2), "epoch": I(1, 2), "intent": E(*INTENTS), "loc": I(1, 3), "secret": I(0, 3), "explicit": B})
M_PROOF = R({"loc": I(1, 3), "state": E("Pending", "Consumed", "Cancelled"), "issuedAt": I(0, 16), "expiresAt": I(0, 16)})
M_BINDING = {"operation": 1, "epoch": 1, "intent": "TheatreSignup"}
MAILBOX_SPECS = {
    "state": R({"durable": R({"proof": M_PROOF, "issued": SET(I(1, 3)), "receipts": MAP("command", I(1, 2), "request", M_REQUEST),
                              "accounts": I(0, MAX)}),
                "now": I(0, 5),
                "plan": OPT("NoPlan", "Planned", R({"command": I(1, 2), "request": M_REQUEST, "expected": M_PROOF, "next": M_PROOF})),
                "session": B, "studio": B, "admin": B}),
    "event": SUM({"NoEvent": None, "Assertion": None, "Inspect": None,
                  "Prepare": R({"command": I(1, 2), "request": M_REQUEST}),
                  "Settle": E("Commit", "Reject", "UnknownApplied", "UnknownLost"),
                  "Cancel": I(1, 3), "Replace": R({"loc": I(1, 3), "fresh": I(1, 3)}), "Clock": I(0, 5)}),
    "outcome": SUM({"NoOutcome": None, "Rejected": E(*MAILBOX_REASONS), "NoOp": E("Inspected", "PlanPrepared", "Replayed"),
                    "Transition": E("ConsumedWithAccount", "CancelledProof", "ReplacedProof"), "UnknownEffect": None, "External": None}),
}
M_BINDING_SPEC = R({key: I(1, 2) if key != "intent" else E(*INTENTS) for key in M_BINDING})
MAILBOX_EVIDENCE = R({"decidedBy": E(*ORIGINS),
                      "calls": R({"planConfirmation": I(0, MAX), "planCancellation": I(0, MAX), "planReplacement": I(0, MAX)}),
                      "binding": M_BINDING_SPEC, "planBinding": OPT(None, None, R({"expected": M_BINDING_SPEC, "next": M_BINDING_SPEC}))})


def mailbox_extra(_expected, actual):
    """The fixture's one proof binding never changes: recovery classes are only wrong inputs."""
    evidence = actual["evidence"]
    require(evidence["binding"] == M_BINDING, "actual proof binding differs from the fixture binding")
    plan = evidence["planBinding"]
    require(plan is None or plan == {"expected": M_BINDING, "next": M_BINDING}, "actual plan binding differs from the fixture binding")
    require((plan is None) == (actual["state"]["plan"] is None), "plan binding observation does not match the held plan")


MAILBOX_CONTRACT = {"schema": "cantos-native-mailbox-proof-observation-v1", "state": MAILBOX_SPECS["state"], "outcome": MAILBOX_SPECS["outcome"],
                    "evidence": MAILBOX_EVIDENCE, "calls": ("planConfirmation", "planCancellation", "planReplacement"),
                    "inputs": ("Clock",), "extra": mailbox_extra}

# ---- cantos.identity.native-session-admission -------------------------------------------------
ADMISSION_REASONS = ("FactMisbound", "NotOnRoster", "IdentityUnavailable", "SubjectUnknown", "FactNotFresh", "Inactive",
                     "EpochMismatch", "VersionMismatch", "NoSession", "SessionRevoked", "SecurityChanged", "NothingRequested",
                     "RevokeWriteFailed")
A_IDENTITY = R({"read": E("Answer", "Unreachable", "Malformed"), "present": B, "active": B, "epoch": I(1, 2), "version": I(1, 8)})
A_FACT = R({"subject": I(1, 2), "purpose": E("Theatre", "Studio"), "echoed": B, "age": E("Fresh", "Stale", "Future"),
            "epoch": I(1, 2), "version": I(1, 8)})
ADMISSION_SPECS = {
    "state": R({"identity": A_IDENTITY, "rostered": B,
                "session": R({"exists": B, "generation": I(0, MAX), "purpose": E("Theatre", "Studio"),
                              "observed": R({"subject": I(0, 2), "epoch": I(0, 2), "version": I(0, 8)}), "revoked": B}),
                "pending": OPT("NoPending", "Requested", E("SubjectMissing", "Disabled", "EpochChanged", "VersionAdvanced", "VersionRegressed"))}),
    "event": SUM({"NoEvent": None, "Assertion": None, "Login": R({"purpose": E("Theatre", "Studio"), "fact": A_FACT}), "Admit": None,
                  "CommitRevoke": B, "Logout": None, "SetIdentity": A_IDENTITY, "SetRoster": B}),
    "outcome": SUM({"NoOutcome": None, "Rejected": E(*ADMISSION_REASONS), "NoOp": E("Admitted", "AlreadyEnded"),
                    "Transition": E("Issued", "Revoked", "LoggedOut"), "External": None}),
}
ADMISSION_EVIDENCE = R({"decidedBy": E(*ORIGINS),
                        "calls": R({"validateFact": I(0, MAX), "securityStateFromResponse": I(0, MAX), "decideIssue": I(0, MAX),
                                    "decideAdmission": I(0, MAX)})})
ADMISSION_CONTRACT = {"schema": "cantos-native-session-admission-observation-v1", "state": ADMISSION_SPECS["state"],
                      "outcome": ADMISSION_SPECS["outcome"], "evidence": ADMISSION_EVIDENCE,
                      "calls": ("validateFact", "securityStateFromResponse", "decideIssue", "decideAdmission"),
                      "inputs": ("SetIdentity", "SetRoster"), "extra": lambda _expected, _actual: None}


def reason(event, outcomes, fields=None, by="core"):
    """`by` names what detects the removed rule: a production seam, the synthetic shell or a harness constant."""
    return {"event": event, "outcomes": [list(item) for item in outcomes], "fields": fields, "by": by}


def adapter(capability, contract, specs, package, test, observer, faults, source_roots, extra_inputs, symbols, shell, reasons):
    return Adapter(capability=capability, schema=contract["schema"], package=package, test=test, observer=observer,
                   variables=("state", "before", "last", "outcome"), decode=lambda raw: decode_trace(raw, specs),
                   compare=lambda cases, observed: compare(contract, cases, observed), faults=faults,
                   source_roots=source_roots, extra_inputs=extra_inputs, symbols=symbols, shell=shell,
                   summary=lambda observed: summarize(contract, observed), reasons=reasons)


MAILBOX_ADAPTER = adapter(
    MAILBOX, MAILBOX_CONTRACT, MAILBOX_SPECS, "cantos-identity",
    "quint_conformance_tests::replay_quint_native_mailbox_proof",
    "services/cantos-identity/src/quint_conformance_tests.rs",
    (("fault-inspect-as-explicit", "inspect-as-explicit", ("run.explicit-confirmation",),
      "a client flag presented as an explicit confirmation escaped original trace assertions"),
     ("fault-skip-commit-recheck", "skip-commit-recheck", ("run.binding-and-window",),
      "a commit that skips the real planner at the commit instant escaped original trace assertions")),
    ("services/cantos-identity", "packages/store-turso"), (),
    ("ProofId::from_bytes", "ProofTime::new", "ProofBinding::new", "MailboxProof::issue", "MailboxProof::plan_confirmation",
     "MailboxProof::plan_cancellation", "MailboxProof::plan_replacement", "ProofTransition::{expected,next}"),
    ("epoch, receipt-replay, secret-class and unknown-operation checks", "lifetime-unique locator set",
     "held plan, compare-and-replace against the current snapshot and commit fate",
     "account counter and constant no-authority projection (the core exposes no authority value)", "owner clock input"),
    {"explicit-only": reason("Prepare", [("Rejected", "NotExplicit")]),
     "consumed-within-bound-window": reason("Settle", [("Rejected", "Expired")]),
     "terminal-sticky": reason("Replace", [("Rejected", "AlreadyCancelled")]),
     "retired-locator-rejected": reason("Prepare", [("Rejected", "ProofChanged")]),
     "plan-has-no-effect": reason("Prepare", [("NoOp", "PlanPrepared")], ["state.durable"], by="shell"),
     "commit-compares-snapshot": reason("Settle", [("Rejected", "StaleSnapshot")], by="shell"),
     "single-account-effect": reason("Prepare", [("Rejected", "AlreadyConsumed")]),
     "no-authority": reason("Settle", [("Transition", "ConsumedWithAccount"), ("UnknownEffect", None)], ["state.session"], by="harness"),
     "only-committed-effects": reason("Settle", [("UnknownEffect", None)], ["state.durable"], by="shell")})
ADMISSION_ADAPTER = adapter(
    ADMISSION, ADMISSION_CONTRACT, ADMISSION_SPECS, "cantos-server",
    "platform::identity_auth::decision::quint_conformance_tests::replay_quint_native_session_admission",
    "services/cantos-server/src/platform/identity_auth/decision/quint_conformance_tests.rs",
    (("fault-fresh-for-every-age", "fresh-for-every-age", ("run.fact-binding",),
      "a fact-age adapter defect escaped original trace assertions"),
     ("fault-admit-against-current", "admit-against-current", ("run.version-advanced", "run.version-regressed", "run.security-changes"),
      "an admission adapter that compares the current state with itself escaped original trace assertions")),
    ("services/cantos-server", "packages/contracts/api", "packages/contracts/identity-rpc", "services/cantos-identity", "packages/store-turso"), (),
    ("validate_fact", "SecurityState::from_response", "decide_issue", "decide_admission"),
    ("Studio roster membership", "single session slot and generation", "absent/unreachable Identity read",
     "requested-versus-committed revocation write and logout", "Identity record and clock inputs"),
    {"issue-needs-fresh-bound-fact": reason("Login", [("Rejected", "FactNotFresh")]),
     "admit-only-exact-active": reason("Admit", [("Rejected", "SecurityChanged")]),
     "mismatch-requests-revoke": reason("Admit", [("Rejected", "SecurityChanged")], ["outcome", "state.pending"]),
     "request-is-not-commit": reason("Admit", [("Rejected", "SecurityChanged")], ["state.session.revoked"], by="shell"),
     "revocation-sticky": reason("Admit", [("Rejected", "SessionRevoked")], by="shell"),
     "outage-never-revokes": reason("Admit", [("Rejected", "IdentityUnavailable")], ["outcome", "state.pending"]),
     "session-changes-only-by-transition": reason("CommitRevoke", [("Rejected", "RevokeWriteFailed")], ["state.session.revoked"], by="shell")})
ADAPTERS = (MAILBOX_ADAPTER, ADMISSION_ADAPTER)
