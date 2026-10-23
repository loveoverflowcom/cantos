# Rights and provenance

> **Scope.** Model, evaluate, persist and test Cantos rights records and provenance: a closed set
> of rights subjects per asset class, grants that record every dimension explicitly, fail-closed
> evaluation for production and distribution use, the commit-time recheck, restriction
> inheritance through generation and mixing, creator scoping of private voice assets, and keeping
> credentials and private evidence out of every published record. Use when adding a rights field,
> an import path, a generated artifact, a rights check, a cache-reuse scope check or a credit.

The product rules are owned elsewhere and are not restated here:
[business rules § Rights and access](../../../../docs/product/business-rules.md#rights-and-access)
(what is recorded, when unknown or expired rights block, inheritance, credential handling, role
scope), rule 15 in [§ Production, caching and costs](../../../../docs/product/business-rules.md#production-caching-and-costs)
(no cross-creator reuse of private voices) and
[pipeline § Freeze production inputs](../../../../docs/product/production-pipeline.md#freeze-production-inputs)
(rights evidence is part of a frozen run). This reference owns how to encode and verify them.
Everything below is a proposal; no rights module or table exists yet.

## 1. Rights subjects are a closed enum over revisions

```rust
// Illustrative, proposed: no `publication::rights` module exists yet.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RightsSubject {
    SourceWork(WorkId),
    Adaptation(AdaptationRevisionRef),
    Voice(VoiceProfileRevisionRef),
    Sound { asset: SoundAssetRevisionRef, kind: SoundKind },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SoundKind {
    Music,
    Ambience,
    Effect,
}
```

- **Closed** so that adding an asset class (cover artwork is the likely first one) breaks every
  evaluator `match` until it is handled. Never add a `RightsSubject::Other(String)`.
- **Revisions, not rows.** A subject names the voice profile revision or sound asset revision a
  run actually froze. A grant for "the theatre room tone" that silently follows a replaced file
  is a different permission than the one reviewed.
- **The subject set is derived, not typed in.** A pure `required_subjects(&FrozenInputs) ->
  BTreeSet<RightsSubject>` walks the frozen run: the work, the adaptation revision, every cast
  voice revision and every cue asset revision. A cue such as `cue-01` (ambience, "Quiet theatre
  room tone") with `asset_id: null` contributes nothing until an asset is selected; a selected
  asset with `rights_record_id: null` contributes a subject that will evaluate to `Missing`.

## 2. A grant records every dimension; unknown is a value

```rust
// Illustrative, proposed.
pub struct RightsGrant {
    id: RightsRecordId,
    subject: RightsSubject,
    holder: RightsHolder,
    evidence: EvidenceRef,              // private object key + checksum, never published
    uses: BTreeSet<Use>,
    languages: Coverage<LanguageTag>,
    territories: Coverage<Territory>,
    term: Term,
    attribution: Attribution,
    restrictions: Restrictions,
    recorded: Recorded,                 // actor, permission scope, ledger sequence
}

pub enum Use {
    Production,                         // paid generation or mixing with this material
    Streaming,
    OfflineDownload,                    // separate: publishing does not imply offline rights
}

pub enum Coverage<T> {
    Unrecorded,                         // nobody established it → blocks
    NotApplicable { stated_by: ActorId, reason: NotApplicableReason },
    Any,
    Only(NonEmptySet<T>),
}

pub enum Term {
    Unrecorded,
    Perpetual,
    Until(Timestamp),                   // half-open: valid while now < until
    Between { from: Timestamp, until: Timestamp },
}
```

Design notes:

- `Unrecorded` and `NotApplicable` are different facts. The business rule says dimensions apply
  "where relevant"; relevance is a recorded statement by an actor, not the absence of a column.
  Which dimensions may be `NotApplicable` for which `RightsSubject` variant is an ADR decision.
- `Use::OfflineDownload` is its own variant because the
  [mobile architecture](../../../../docs/architecture/mobile.md#downloads-and-access-rights) states
  that permission to publish does not grant offline redistribution.
- A grant is effective only with a separate `RightsReview { grant, decision, reviewer, at }`
  record. Imported material starts unreviewed, matching the `pending_review` status in the
  [illustrative fixture](../../../../services/cantos-server/contracts/studio/script-ir/examples/episode-draft.json).
- Withdrawal is a new `RightsRevocation { grant, effective_at, actor, reason }` row. Grants,
  reviews and revocations are append-only; nothing in the ledger is updated in place
  ([immutability](../../cantos-engineering/references/immutability.md)).
- Deserialize grants through a raw DTO and `TryFrom`; an unknown `Use`, `Coverage` or `Term` tag
  is a decode error, never a default
  ([boundary hardening](../../cantos-engineering/references/boundary-hardening.md)).

## 3. Evaluation is pure, total and returns every blocker

```rust
// Illustrative, proposed.
pub struct IntendedUse {
    pub purpose: Use,
    pub language: LanguageTag,
    pub territories: Coverage<Territory>,
    pub creator: CreatorScope,
    pub at: Timestamp,                  // one instant, read once by the shell
}

pub fn evaluate_rights(
    required: &BTreeSet<RightsSubject>,
    ledger: &RightsLedger,
    intended: &IntendedUse,
) -> Result<ClearedRights, Vec<RightsBlocker>> {
    let blockers: BTreeSet<RightsBlocker> = required
        .iter()
        .flat_map(|subject| subject_blockers(subject, ledger, intended))
        .collect();

    if blockers.is_empty() {
        Ok(ClearedRights::from_ledger(required, ledger, intended))
    } else {
        Err(blockers.into_iter().collect())
    }
}
```

`RightsBlocker` variants, each with the identifiers a creator needs to fix it: `Missing {
subject }`, `PendingReview { grant }`, `ReviewRejected { grant }`, `Revoked { grant,
effective_at }`, `NotYetValid { grant, from }`, `Expired { grant, until }`, `UseNotGranted {
grant, purpose }`, `LanguageNotCovered { grant, language }`, `TerritoryNotCovered { grant,
territory }`, `DimensionUnrecorded { grant, dimension }`, `ForeignCreatorVoice { voice, owner }`
and `InheritedRestriction { artifact, restriction }`.

- A subject is cleared when **one** effective grant covers every dimension. Never combine two
  partial grants (one covering Vietnamese, another covering the territory) into coverage unless
  an ADR records that the rights holder permits it; that is a legal question, not a set union.
- When no grant covers a subject, emit the blockers of every candidate grant, so the Studio can
  show what each one lacks.
- `ClearedRights` is evidence: it carries the grant IDs used, the attribution obligations and the
  merged restrictions, so manifest assembly and the publication gate read credits from it rather
  than re-querying. It has a private constructor and no `Deserialize`.
- The intended territory for Theatre distribution is not defined by the product docs yet. Until
  it is, model it as `Coverage::Any` (worldwide), which makes every territory-limited grant
  block. Fail closed; do not guess a territory.

## 4. Two checks, one evaluator, the second inside the commit

| Check | Caller | `purpose` | What it records |
|---|---|---|---|
| before paid generation | production run freeze and cost reservation in [`cantos-production-pipeline`](../../cantos-production-pipeline/SKILL.md) | `Production` | the `ClearedRights` grant IDs inside the frozen inputs |
| readiness preview | the Studio readiness endpoint | `Streaming` (+ `OfflineDownload` when offered) | nothing; explanation only |
| publication commit | the publication transaction | same as preview | the grant IDs in the release manifest provenance |

The preview and the commit can disagree: a revocation or review rejection may commit between
them. The evaluation that authorizes publication therefore reads the ledger **inside** the
commit transaction, under a concurrency control that rights writers also respect. Two candidate
designs, to be chosen in an ADR and tested on real PostgreSQL
([persistence](../../cantos-engineering/references/persistence.md)):

| Design | Mechanism | Cost |
|---|---|---|
| per-episode publication lock | the commit locks the episode's publication row; every rights write locks the rows of all episodes whose frozen inputs reference the subject | a work-level revocation touches many episodes; the reverse index must be complete |
| serializable isolation | publication commits and rights writes run `SERIALIZABLE`; the shell retries serialization failures a bounded number of times | retry logic in the shell; contention on popular assets |

Pass the transaction's single timestamp into `IntendedUse::at`; never call a wall clock inside
the core. Expiry is time-driven and writes no row, so no lock can catch it: a scheduled sweep
evaluates active releases at `now` and emits findings. What an expired grant does to an
already-active release (automatic retraction or an alert to the owner) is not defined by the
product docs; treat it as an open product decision and report it.

## 5. Provenance and restriction inheritance

Every imported or generated artifact has an immutable provenance record written when the
pipeline accepts it. The pipeline knows the producer and the attempt; this reference defines
what provenance must contain for rights and how restrictions propagate.

```rust
// Illustrative, proposed.
pub struct Provenance {
    pub artifact: ArtifactChecksum,
    pub producer: Producer,             // Import { tool, version } | Provider { provider, model,
                                        // model_version: Pinned(..) | Unpinned } | Mixer { tool, profile }
    pub attempt: Option<AttemptId>,
    pub inputs: BTreeSet<ProvenanceInput>,  // artifacts and rights subjects it was made from
}

pub fn inherited_restrictions(
    inputs: &[&Restrictions],
    producer_terms: &Restrictions,
) -> Restrictions {
    inputs
        .iter()
        .copied()
        .fold(producer_terms.clone(), |acc, input| acc.union(input))
}
```

Laws, each a property test over generated restriction sets (proptest is a candidate):

| Law | Statement | Failure it catches |
|---|---|---|
| monotone | adding an input never removes a restriction | a mix with a restricted track loses the restriction |
| order-independent | any permutation of inputs gives the same result | restrictions depend on cue order |
| idempotent | listing an input twice changes nothing | duplicate cue references distort credits |
| no broadening | allowed uses of the output ⊆ intersection of the inputs' allowed uses | a dialogue render allows `OfflineDownload` its voice forbids |

"An output's existence does not establish its distribution rights" becomes a code property:
there is no `From<AcceptedArtifact> for RightsGrant` and no path that marks a subject cleared
because an artifact exists. The publication gate raises `ProvenanceMissing { artifact }` for any
manifest asset without provenance, and `InheritedRestriction` when the merged restrictions
forbid the intended use.

## 6. Private voice assets stay inside their creator scope

The speech fingerprint decides whether two requests would produce the same audio; it is not an
authorization ([fingerprints](../../cantos-production-pipeline/references/fingerprints-and-invalidation.md)).
After a fingerprint hit, cache reuse must also pass:

```rust
// Illustrative, proposed.
pub fn may_reuse(artifact: &AcceptedArtifact, requester: &CreatorScope) -> Result<(), ReuseRefusal>
```

It refuses a private voice artifact whose owner scope differs from the requester's, and any
artifact whose provenance restrictions do not cover the requester's intended use. The docs do not
define which creators are "related" (team, organization); until the identity and role model
does, every creator account is its own scope. Test: two creators, identical fingerprints, one
private voice; the second request generates fresh audio and the first creator's artifact is
never returned or listed.

## 7. Secrets and private evidence never cross into published records

- Permission evidence (contracts, consent recordings, emails) is a private object referenced by
  `EvidenceRef { key, checksum }`. A release manifest carries rights record IDs and the public
  credit text from `ClearedRights`, never evidence keys or holder contact details.
- Provider and storage credentials are typed secrets with redacted `Debug` and no `Serialize`
  (the `secrecy` crate is a candidate). They are loaded in the shell and never enter a fact
  struct, a Script IR document, a manifest, an outbox payload or a log line.
- Oracle: run the publication flow with a sentinel credential such as
  `cantos-test-secret-do-not-log`; serialize every manifest, outbox payload and API response the
  flow produced and capture its logs; assert the sentinel and any `X-Amz-Signature` or
  `Signature=` substring are absent. Label: `example-tested`.

## Rule cards

| Rule | Why | Good | Counterexample | Oracle | Status · exception |
|---|---|---|---|---|---|
| Unknown is a blocking value | absence reads as "fine" in most code | `Coverage::Unrecorded` → `DimensionUnrecorded` | `territory: Option<String>` with `None` skipped | one table row per dimension | proposed · none |
| Evaluate at commit, not only at preview | revocation between preview and commit | re-run `evaluate_rights` on in-transaction reads | trusting the readiness response's `ok: true` | interleaved revoke-then-commit on real PostgreSQL | proposed · none |
| One grant covers a subject | combining partial grants is a legal claim | per-grant coverage check | union of two grants' territories | example with two partial grants → blocked | proposed · an ADR naming the holder's permission |
| Restrictions only accumulate | laundering through derivation | `inherited_restrictions` fold | mixer output with empty restrictions | the four laws as properties | proposed · none |
| Reuse requires scope, not only fingerprint | private voice leaks across creators | `may_reuse` after a fingerprint hit | cache lookup by fingerprint alone | two-creator example | proposed · public stock voices whose grant allows any creator |
| Evidence and secrets stay private | published manifests are public | rights IDs and credit text only | `evidence_url` in the manifest | sentinel scan | proposed · none |

## Tests worth writing first

| Test (named like a theorem) | Level |
|---|---|
| `missing_grant_for_selected_ambience_blocks_with_missing_subject` | `example-tested` |
| `grant_expiring_exactly_at_evaluation_instant_is_expired` (and one tick before is valid) | `example-tested` |
| `unrecorded_territory_blocks_even_when_language_matches` | `example-tested` |
| `streaming_grant_does_not_cover_offline_download` | `example-tested` |
| `review_rejection_after_preview_blocks_the_commit` | `integration-tested` (real PostgreSQL) |
| `inherited_restrictions_are_monotone_order_independent_and_idempotent` | `property-tested` |
| `private_voice_artifact_is_not_reused_for_another_creator` | `example-tested` |
| `unknown_use_tag_in_stored_grant_fails_decode_with_row_id` | `example-tested` |

## Gotchas

- Vietnamese holder names and credits (`Người dẫn chuyện`, `Ánh đèn cuối sân khấu`) are compared
  and stored NFC-normalized; a credit that renders identically but differs in normalization fails
  an attribution equality check. See the script IR
  [Vietnamese text reference](../../cantos-script-ir/references/vietnamese-text.md).
- A grant's language is a BCP 47 tag (`vi-VN`, `vi`); decide whether `vi` covers `vi-VN` in the
  evaluator and test it, rather than comparing strings.
- Rights evaluation is cheap; do not cache `ClearedRights` across transactions. A cached result
  is a stale proof.
