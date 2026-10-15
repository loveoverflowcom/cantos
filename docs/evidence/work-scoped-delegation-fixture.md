# Work-scoped delegation — exact disposable fixture appendix

This is a **documentation-only archival listing** for the
[decision-pending research evidence](work-scoped-delegation-research.md), not production code,
a credential SDK, a new repository command or an automated regression gate. It preserves the
exact public experiment so that a fresh checkout can replay it without session-local files.
Do not import this reducer/parser into Cantos or use its deterministic keys/tokens anywhere real.

The listing's LF-encoded bytes, including its final newline, have SHA-256
`183fa799d1898e8b9c1d08ad06a86d7aa7c966954acb249e9467c6fdc50c434c`.
Original run: Python 3.12.14 and `cryptography` 50.0.0. Replay requires an already-installed
Python 3 and compatible Ed25519 support from `cryptography`; missing support is **NOT_RUN**,
not permission to install anything automatically. Record actual versions for every replay.
The assertions require ordinary Python mode, not `python -O` or `PYTHONOPTIMIZE`.

## Hash-checked replay from a fresh checkout

Run from the repository root. This stdlib extraction verifies the exact listing before execution,
uses a new temporary directory, and leaves the original observation/results untouched. Only the
public synthetic experiment runs; it reads no credentials, connects to no service and writes
`results.json` next to its extracted copy. The temporary path is printed for inspection/disposal.
These commands reproduce a research record, not a competing Cantos build/test/run interface.

```sh
python3 - <<'PY'
import hashlib
import json
from pathlib import Path
import subprocess
import sys
import tempfile

if sys.flags.optimize:
    raise SystemExit("Refuse optimized Python: fixture assertions would be disabled")
doc = Path("docs/evidence/work-scoped-delegation-fixture.md").read_bytes()
begin = b"<!-- issue113-fixture-source:start -->\n```python\n"
end = b"```\n<!-- issue113-fixture-source:end -->"
if doc.count(begin) != 1 or doc.count(end) != 1:
    raise SystemExit("Missing or ambiguous fixture source boundaries")
source = doc.split(begin, 1)[1].split(end, 1)[0]
expected = "183fa799d1898e8b9c1d08ad06a86d7aa7c966954acb249e9467c6fdc50c434c"
if hashlib.sha256(source).hexdigest() != expected:
    raise SystemExit("Fixture source hash mismatch")
run_dir = Path(tempfile.mkdtemp(prefix="cantos-issue113-replay-"))
fixture = run_dir / "fixtures.py"
fixture.write_bytes(source)
print("Research output directory:", run_dir, flush=True)
subprocess.run([sys.executable, str(fixture)], check=True)
result = json.loads((run_dir / "results.json").read_text())
if result["source_sha256"] != expected or result["asserted_cases"] != 64 or result["subset_oracle_cases"] != 8:
    raise SystemExit("Unexpected source/case counts")
print("PASS: 64 exact outcomes and 8 fixed subset cases; timing remains a local observation")
PY
```

Expected stable outputs: 64 named outcomes, eight fixed subset examples, opaque size 43 and
signed size 751 ASCII bytes. Per-round nanoseconds are intentionally not golden values. The
[research record](work-scoped-delegation-research.md#observed-exact-assertions) defines the
partitions and explicitly excludes Cantos implementation, SQL locks, HTTP, provider or complete
wire interoperability. A subset predicate over eight examples is not an attenuation proof.

## Known illustrative limits

The verifier is deliberately not a hardened general parser. For example, a correctly signed
JSON array payload (`[]`) reaches `.get` and raises `AttributeError`; this second-session
negative control is outside the 64 passing baseline cases. There is no production request path
using this fixture, and no claim that all hostile inputs become closed denials. Preserve this
historical source rather than polishing it into an unofficial security implementation. A selected
library's shape/type/resource-limit rejection and official vectors need their own later campaign.
Boolean identity/current-fact inputs are supplied by the test, not an authenticated presenter,
real resource resolver, SQL fence or remote identity service.

## Exact historical experiment source

<!-- issue113-fixture-source:start -->
```python
"""Disposable issue113 mechanics fixture. Synthetic values, not Cantos code or protocol SDK."""
import base64
import copy
from dataclasses import dataclass, replace
from enum import Enum
import hashlib
import hmac
import json
from pathlib import Path
import platform
import statistics
import time

import cryptography
from cryptography.exceptions import InvalidSignature
from cryptography.hazmat.primitives.asymmetric.ed25519 import Ed25519PrivateKey


class Outcome(str, Enum):
    ALLOW = "allow"
    CREDENTIAL = "invalid_credential"
    PURPOSE = "wrong_purpose"
    IDENTITY = "unknown_delegate"
    INACTIVE = "inactive_actor"
    REVOKED = "revoked"
    ANCESTRY = "foreign_resource"
    ACTION = "action_denied"
    TIME = "outside_window"
    OWNER = "grantor_no_longer_owner"
    ARCHIVED = "archived"
    BINDING = "pin_mismatch"
    DOMAIN = "independent_domain_denied"
    MIXED = "mixed_credentials"


WORK = "11111111-1111-4111-8111-111111111111"
UNIT = "22222222-2222-4222-8222-222222222222"
REVISION = "33333333-3333-4333-8333-333333333333"
FOREIGN = "44444444-4444-4444-8444-444444444444"
ISSUER = "https://issuer.example.invalid/delegation-fixture"
AUDIENCE = "cantos-work-fixture"
PURPOSE = "work-delegation-fixture"
# Public deterministic test seed and bytes. NO randomness/entropy or live credential claim.
KEY = Ed25519PrivateKey.from_private_bytes(hashlib.sha256(b"issue113 public test key").digest())
PUBLIC = KEY.public_key()
SECRET = hashlib.sha256(b"issue113 public opaque fixture").digest()
LOOKUP_DOMAIN = b"issue113/opaque-fixture/v1\n"
HASH_KEY = hashlib.sha256(b"issue113 public lookup key").digest()


@dataclass(frozen=True)
class Grant:
    grant_id: str = "55555555-5555-4555-8555-555555555555"
    delegate: str = "synthetic-agent-1"
    grantor: str = "synthetic-owner-1"
    work: str = WORK
    unit: str = UNIT
    revision: str = REVISION
    actions: tuple = ("content:read", "content:write")
    nbf: int = 100
    exp: int = 200
    purpose: str = PURPOSE


@dataclass(frozen=True)
class Facts:
    now: int = 150
    known_delegate: bool = True
    delegate_active: bool = True
    grantor_active: bool = True
    grant_revoked: bool = False
    parent_revoked: bool = False
    current_owner: str = "synthetic-owner-1"
    archived: bool = False
    work: str = WORK
    unit: str = UNIT
    revision: str = REVISION
    domain_allowed: bool = True


@dataclass(frozen=True)
class Request:
    work: str = WORK
    unit: str = UNIT
    revision: str = REVISION
    action: str = "content:write"
    studio_cookie: bool = False


G = Grant()
F = Facts()
R = Request()


def b64(data):
    return base64.urlsafe_b64encode(data).rstrip(b"=").decode("ascii")


def unb64(text):
    if not text or any(c not in "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_" for c in text):
        raise ValueError("noncanonical base64url")
    result = base64.urlsafe_b64decode(text + "=" * (-len(text) % 4))
    if b64(result) != text:
        raise ValueError("noncanonical base64url")
    return result


def canonical(value):
    return json.dumps(value, sort_keys=True, separators=(",", ":")).encode("ascii")


def no_duplicates(pairs):
    out = {}
    for key, value in pairs:
        if key in out:
            raise ValueError("duplicate member")
        out[key] = value
    return out


def claims(g):
    return {"iss": ISSUER, "aud": AUDIENCE, "sub": g.delegate, "grantor": g.grantor,
            "jti": g.grant_id, "work": g.work, "unit": g.unit, "revision": g.revision,
            "actions": list(g.actions), "nbf": g.nbf, "exp": g.exp, "purpose": g.purpose}


def signed_token(g=G, changes=None, header_changes=None, payload_bytes=None):
    head = {"alg": "EdDSA", "typ": PURPOSE, "kid": "fixture-only"}
    head.update(header_changes or {})
    body = claims(g)
    body.update(changes or {})
    message = (b64(canonical(head)) + "." + b64(payload_bytes or canonical(body))).encode("ascii")
    return message.decode("ascii") + "." + b64(KEY.sign(message))


def signed_proof(token):
    try:
        if len(token) > 4096:
            return Outcome.CREDENTIAL
        header, payload, signature = token.split(".")
        head = json.loads(unb64(header), object_pairs_hook=no_duplicates)
        if head != {"alg": "EdDSA", "typ": PURPOSE, "kid": "fixture-only"}:
            return Outcome.PURPOSE
        PUBLIC.verify(unb64(signature), (header + "." + payload).encode("ascii"))
        c = json.loads(unb64(payload), object_pairs_hook=no_duplicates)
        if c.get("iss") != ISSUER or c.get("aud") != AUDIENCE or c.get("purpose") != PURPOSE:
            return Outcome.PURPOSE
        # Proof does NOT trust sub, current ownership or live rights: compare immutable grant.
        if c != claims(G):
            return Outcome.CREDENTIAL
        return G
    except (ValueError, TypeError, InvalidSignature):
        return Outcome.CREDENTIAL


OPAQUE = b64(SECRET)
DB = {hashlib.sha256(LOOKUP_DOMAIN + SECRET).digest(): G}


def opaque_proof(token):
    try:
        raw = unb64(token)
        if len(raw) != 32:
            return Outcome.CREDENTIAL
        return DB.get(hashlib.sha256(LOOKUP_DOMAIN + raw).digest(), Outcome.CREDENTIAL)
    except ValueError:
        return Outcome.CREDENTIAL


def policy(g, facts, request):
    if isinstance(g, Outcome):
        return g
    if request.studio_cookie:
        return Outcome.MIXED
    if g.purpose != PURPOSE:
        return Outcome.PURPOSE
    if not facts.known_delegate:
        return Outcome.IDENTITY
    if not facts.delegate_active or not facts.grantor_active:
        return Outcome.INACTIVE
    if facts.grant_revoked or facts.parent_revoked:
        return Outcome.REVOKED
    if g.grantor != facts.current_owner:
        return Outcome.OWNER
    if (request.work, request.unit) != (facts.work, facts.unit) or (g.work, g.unit) != (facts.work, facts.unit):
        return Outcome.ANCESTRY
    if request.revision != facts.revision or g.revision != facts.revision:
        return Outcome.BINDING
    if request.action not in g.actions:
        return Outcome.ACTION
    if not g.nbf <= facts.now < g.exp:
        return Outcome.TIME
    if facts.archived:
        return Outcome.ARCHIVED
    if not facts.domain_allowed:
        return Outcome.DOMAIN
    return Outcome.ALLOW


SCENARIOS = [
    ("allowed_write", F, R, Outcome.ALLOW),
    ("allowed_read", F, replace(R, action="content:read"), Outcome.ALLOW),
    ("foreign_work", F, replace(R, work=FOREIGN), Outcome.ANCESTRY),
    ("foreign_unit", F, replace(R, unit=FOREIGN), Outcome.ANCESTRY),
    ("foreign_revision", F, replace(R, revision=FOREIGN), Outcome.BINDING),
    ("changed_authoritative_ancestry", replace(F, work=FOREIGN), R, Outcome.ANCESTRY),
    ("publish_denied", F, replace(R, action="publication:publish"), Outcome.ACTION),
    ("membership_denied", F, replace(R, action="membership:write"), Outcome.ACTION),
    ("issuance_denied", F, replace(R, action="grant:issue"), Outcome.ACTION),
    ("admin_denied", F, replace(R, action="admin:revoke"), Outcome.ACTION),
    ("unknown_subject", replace(F, known_delegate=False), R, Outcome.IDENTITY),
    ("delegate_disabled", replace(F, delegate_active=False), R, Outcome.INACTIVE),
    ("grantor_disabled", replace(F, grantor_active=False), R, Outcome.INACTIVE),
    ("grant_revoked", replace(F, grant_revoked=True), R, Outcome.REVOKED),
    ("parent_revoked", replace(F, parent_revoked=True), R, Outcome.REVOKED),
    ("owner_changed", replace(F, current_owner="synthetic-owner-2"), R, Outcome.OWNER),
    ("before_nbf", replace(F, now=99), R, Outcome.TIME),
    ("at_nbf", replace(F, now=100), R, Outcome.ALLOW),
    ("before_exp", replace(F, now=199), R, Outcome.ALLOW),
    ("at_exp", replace(F, now=200), R, Outcome.TIME),
    ("archived", replace(F, archived=True), R, Outcome.ARCHIVED),
    ("independent_editorial_denial", replace(F, domain_allowed=False), R, Outcome.DOMAIN),
    ("mixed_cookie_bearer", F, replace(R, studio_cookie=True), Outcome.MIXED),
]


def assert_case(name, actual, expected, results):
    assert actual == expected, (name, actual, expected)
    results.append({"case": name, "outcome": actual.value})


def timing(fn, count):
    samples = []
    for _ in range(5):
        start = time.perf_counter_ns()
        for _ in range(count):
            fn()
        samples.append((time.perf_counter_ns() - start) / count)
    return {"iterations_per_round": count, "rounds": 5, "median_ns": round(statistics.median(samples)),
            "min_ns": round(min(samples)), "max_ns": round(max(samples))}


results = []
SIGNED = signed_token()
for name, proof, token in [("A", opaque_proof, OPAQUE), ("B", signed_proof, SIGNED)]:
    for case, facts, request, expected in SCENARIOS:
        assert_case(name + ":" + case, policy(proof(token), facts, request), expected, results)
    # Emulate I/O gap without a real store/lock; final refusal means zero synthetic commits.
    before = policy(proof(token), F, R)
    after = policy(proof(token), replace(F, parent_revoked=True), R)
    commits = 1 if after == Outcome.ALLOW else 0
    assert before == Outcome.ALLOW and after == Outcome.REVOKED and commits == 0
    results.append({"case": name + ":synthetic_io_gap_revocation", "outcome": after.value, "commits": commits})

for name, token in [("tampered", SIGNED[:-1] + ("A" if SIGNED[-1] != "A" else "B")),
                    ("truncated", SIGNED[:-10]), ("invalid", "not-a-token")]:
    assert_case("B:" + name, signed_proof(token), Outcome.CREDENTIAL, results)
for name, token in [("algorithm_none", signed_token(header_changes={"alg": "none"})),
                    ("unknown_key", signed_token(header_changes={"kid": "unknown"})),
                    ("wrong_type", signed_token(header_changes={"typ": "at+jwt"})),
                    ("wrong_issuer", signed_token(changes={"iss": "https://attacker.example.invalid"})),
                    ("wrong_audience", signed_token(changes={"aud": "cantos-admin-api"})),
                    ("wrong_purpose", signed_token(changes={"purpose": "producer-read"}))]:
    assert_case("B:" + name, signed_proof(token), Outcome.PURPOSE, results)
assert_case("B:duplicate_sub", signed_proof(signed_token(payload_bytes=b'{"sub":"a","sub":"b"}')), Outcome.CREDENTIAL, results)
assert_case("B:changed_signed_scope", signed_proof(signed_token(changes={"work": FOREIGN})), Outcome.CREDENTIAL, results)
for name, token in [("malformed", "!"), ("unknown", b64(b"\x00" * 32)), ("short", b64(b"\x00" * 31))]:
    assert_case("A:" + name, opaque_proof(token), Outcome.CREDENTIAL, results)
# Signature proof remains valid after external revocation; crypto alone is insufficient.
assert isinstance(signed_proof(SIGNED), Grant)
assert_case("B:proof_valid_but_online_revoked", policy(signed_proof(SIGNED), replace(F, grant_revoked=True), R), Outcome.REVOKED, results)
# Same bearer bytes in another purpose domain do not address the Work-grant lookup.
assert hashlib.sha256(b"cantos/producer-read/v1\n" + SECRET).digest() not in DB
results.append({"case": "A:domain_separation", "outcome": "no_lookup_match"})

# Non-cryptographic bounded subset oracle only. No Biscuit/Macaroons implementation claim.
def narrower(parent, child):
    return (parent.work == child.work and parent.unit == child.unit and parent.revision == child.revision
            and set(child.actions) <= set(parent.actions) and child.nbf >= parent.nbf and child.exp <= parent.exp
            and child.nbf < child.exp)
subset_cases = [
    ("read_only_child", replace(G, actions=("content:read",), nbf=120, exp=180), True),
    ("restored_publish", replace(G, actions=("publication:publish",)), False),
    ("extended_expiry", replace(G, exp=201), False),
    ("earlier_start", replace(G, nbf=99), False),
    ("changed_work", replace(G, work=FOREIGN), False),
    ("changed_unit", replace(G, unit=FOREIGN), False),
    ("changed_revision", replace(G, revision=FOREIGN), False),
    ("empty_window", replace(G, nbf=200), False),
]
for name, child, expected in subset_cases:
    assert narrower(G, child) == expected

out = {
    "scope": "disposable synthetic mechanics; no Cantos implementation, SQL, HTTP, provider or wire interoperability",
    "source_sha256": hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
    "python": platform.python_version(), "cryptography": cryptography.__version__,
    "test_results": results, "asserted_cases": len(results), "subset_oracle_cases": len(subset_cases),
    "token_bytes": {"opaque_base64url_32_raw_bytes": len(OPAQUE), "signed_compact_fixture": len(SIGNED)},
    "mechanics_timings": {
        "sha256_32_byte_secret_domain": timing(lambda: hashlib.sha256(LOOKUP_DOMAIN + SECRET).digest(), 10000),
        "hmac_sha256_32_byte_secret_domain": timing(lambda: hmac.digest(HASH_KEY, LOOKUP_DOMAIN + SECRET, "sha256"), 10000),
        "opaque_lookup": timing(lambda: opaque_proof(OPAQUE), 10000),
        "signed_signature_parse_and_static_binding": timing(lambda: signed_proof(SIGNED), 2000),
    },
    "not_run": ["BLAKE3", "PASETO", "Rust crate compatibility", "Biscuit/Macaroons crypto", "SQL locks", "HTTP", "S3", "live identity", "Quint", "Kani"],
}
Path(__file__).with_name("results.json").write_text(json.dumps(out, indent=2) + "\n")
print(json.dumps({k: v for k, v in out.items() if k != "test_results"}, indent=2))
```
<!-- issue113-fixture-source:end -->
