# Cantos Forge

Producer-side library/CLI for Cantos content production, governed by
[ADR 0022](../../docs/decisions/0022-cantos-forge-producer-cli.md) from the
[Forge vision issue](https://github.com/loveoverflowcom/cantos/issues/82). Forge runs in the
producer's environment, **outside the Cantos API runtime**: it manages local artifact
revisions, evaluation evidence and local handoff bundles for AI hosts and human producers.
The API and Theatre never execute generation. Studio owns the planned controls for externally
executed Forge operations; Forge never publishes.

Cantos Forge is unrelated to the external *Narrative Forge* project. The rule that Cantos code
never imports Narrative Forge internals applies to Forge too.

## Status

| Part | State |
| --- | --- |
| Crate, workspace membership, `cargo xtask build cantos-forge` | implemented |
| Pure domain core (below), including a strict Ogg Opus inspector | implemented, example-, property- and differentially-tested |
| Host-assisted local workflow: `workspace init`, `artifact add/list/show`, `validate`, `handoff export` | implemented (filesystem workspace, local bundle) |
| `capabilities [--json]` | implemented: reports exactly the commands this build has |
| Instruction bundles, `plan`/`run`/`resume`, AI-review and human-review evidence commands | proposed (rest of #82's foundation) |
| Provider/tool adapters, budget and attempt recovery | proposed (#82 PR 2); no provider is configured or called |
| Authorized handoff to the Cantos API (`submit`) | proposed (#82 PR 3); no HTTP client exists |
| Studio-controlled local/Firebase Functions execution and private Drive drafts | owner-selected direction in #203; runtime adapter, provider execution and Drive I/O are not implemented |
| MCP or other general daemon/server mode | deferred; #203's bounded Functions adapter is the concrete next runtime consumer |

This build of Forge **does not make audio**. In the host-assisted workflow the AI host or a person produces
text and audio with its own tools, then registers the exact bytes. Nothing here creates a
server identity, a Production Brief, a submission, an admission or a release. A bundle is local
files; `capabilities` reports the API handoff and provider-managed workflows as `unsupported`.

The [2026-10-16 direction](../../docs/decisions/0022-cantos-forge-producer-cli.md) keeps one Rust
producer core for CLI and Functions, Studio-owned configuration/status/preview/acceptance, and
private personal Drive for remote drafts (filesystem for offline local work). It adds no current
command, Drive access, Gemini integration, cloud deployment or automatic API submission.

## The host-assisted workflow

```sh
cantos-forge workspace init --path ./forge
cantos-forge artifact add --workspace ./forge --kind manuscript-original --file story.txt \
  --producer "AI host session" --rights original-work
cantos-forge artifact add --workspace ./forge --kind audio --file chapter-1.ogg \
  --producer "macOS say + afconvert" --rights original-work --input art_<story-id>@1
cantos-forge validate --workspace ./forge --artifact art_<audio-id> --profile opus-mono-24-v1
cantos-forge handoff export --workspace ./forge --artifact art_<audio-id> \
  --profile opus-mono-24-v1 --out ./bundles/chapter-1
```

Add `--json` to any command for one machine-readable envelope. Each step is idempotent: an
identical replay creates nothing. Choices that could silently change meaning have **no
default**: the rights claim, the audio profile and the producer label are always stated.

| Command | Does | Never |
| --- | --- | --- |
| `workspace init --path` | creates a workspace in a missing or empty directory | writes into a directory that holds other files |
| `artifact add --kind --file --producer --rights [--artifact] [--input art@rev]…` | registers the file's exact bytes as revision 1, or the next revision of `--artifact`; pins inputs by recorded digest | accepts an artifact id you chose, a digest you typed or an unknown input |
| `artifact list`, `artifact show --artifact` | revisions, input freshness (current or stale), evidence and the three evaluation layers | treats a technical pass as a review |
| `validate --artifact [--profile]` | measures the newest revision and records deterministic evidence (Ogg Opus profile for audio, UTF-8 for text) | measures audio without a producer-chosen profile |
| `handoff export --artifact --profile --out` | after a current passing check, fresh inputs and a stated rights claim, writes the exact audio bytes and `bundle.json` to a new directory | uploads, submits or approves anything |

Kinds: `manuscript-original`, `manuscript-collected`, `script-ir`, `audio`, `report`,
`instruction-bundle`. Only manuscripts and audio have a deterministic check today; Script IR
validation belongs to the shared validator that does not exist as a package yet. Rights:
`unstated`, `original-work`, `licensed` and `permitted-source` (these two need
`--rights-reference`). A claim is never verified rights.

## Layout

```text
src/
  main.rs            composition root: the filesystem runtime, the response writer, the exit status
  lib.rs
  domain/            pure core; no I/O, clock, randomness, process, CLI or serde
    identity.rs      LocalId<K>: ArtifactId, EvidenceId; kinds cannot be mixed
    digest.rs        Sha256Digest of exact bytes
    workspace_path.rs WorkspacePath: lexically cannot leave the workspace root
    label.rs         bounded labels (producer, claim reference, check, model, reviewer)
    artifact.rs      immutable ArtifactRevision, type-specific payloads, provenance, rights
                     claims, input pins, the idempotent registration decision and `replay`
    lineage.rs       RevisionHistory and pin freshness: current, stale or violation
    evaluation.rs    evidence and three independent layers: deterministic, AI, human
    audio_profile.rs the four intake profiles, checked against the contract's own table
    ogg_opus.rs      strict Ogg Opus framing, CRC, granules and facts from exact bytes
    checks.rs        deterministic checks (Ogg Opus profile, UTF-8 text) and their measurements
    handoff.rs       export eligibility: every refusal reason, never an approval
  application/       use cases over ports: register, validate, view, export; in-memory fakes in tests
  adapters/          filesystem workspace, local bundle writer, OS random identities
  wire.rs            JSON shapes of records, manifests and results; refuses unknown fields
  interfaces/        clap arguments, command execution, typed replies and the exit status
tests/
  architecture.rs    domain, application and wire import no I/O, framework or outer layer (lexical)
  audio_profile_contract.rs  Forge's profiles against packages/contracts/studio-audio-intake-v1.md
  cli.rs             the built binary end to end on a real directory
  support/ogg_fixture.rs     a small Ogg Opus writer with its own bitwise CRC, shared with unit tests
```

Dependencies point inward: `interfaces` and `adapters` use `application`, `application` uses
`domain`, and `wire` sits beside them for the JSON shapes. The crate depends only on pinned
workspace dependencies (`clap`, `getrandom`, `serde`, `serde_json`, `sha2`; `proptest` and
`tempfile` for tests). It never depends on `services/` or another app; `cargo test -p xtask`
checks every workspace dependency edge points inward.

## The workspace on disk

```text
<root>/forge-workspace.json                marker with the format name
<root>/artifacts/<art_id>/r00000001.json   one immutable revision record per file
<root>/evidence/<evd_id>.json              one append-only evidence record per file
<root>/blobs/sha256-<hex>                  the exact bytes, named by their digest
```

A file is written under a temporary name, then hard-linked to its final name: a reader never
sees half a record, a crash leaves no truncated record, and a writer that loses a race gets a
typed `conflict` instead of overwriting. Reading fails closed: a symbolic link, an unknown field
or file, a record that claims another revision number, a history registration would refuse, or
bytes that no longer hash to their name each stop the command with `workspace_corrupt` and the
exact record, and nothing is changed. Symlink and time-of-check races against a hostile local
process, hard links, and case- or normalization-insensitive file systems are not claims of this
slice; the workspace is the producer's own directory.

## Core invariants

| Invariant | Barrier | Evidence |
| --- | --- | --- |
| an artifact identity is never used as an evidence identity | `LocalId<K>` phantom kind and text prefix | type-enforced (compile-fail doctest), example-tested |
| Forge, never the caller, chooses a new artifact's identity | `Target::New` / `Ids` port | example-tested |
| a digest names exact bytes; NFC and NFD text differ | `Sha256Digest` with one canonical form | example-tested (NIST vectors), property-tested round trip |
| a workspace path cannot name anything outside the root | `WorkspacePath::parse` | example-tested per refusal, property-tested over generated paths |
| a revision is written once; a change is a successor; a replay creates nothing | private fields, `decide_registration` | example-tested |
| restored history is exactly what registration would have produced | `artifact::replay` on every load | example-tested, tamper-tested on disk |
| kind and manuscript origin never change across revisions; a revision never pins itself | `RevisionError` variants | example-tested |
| a pin takes its digest from recorded history, never from the caller | `register_artifact` | example-tested |
| a new input revision marks dependants stale; unrelated revisions do not | `lineage::assess` | property-tested |
| a pin contradicting history fails closed with every violation | `PinViolation` | example- and property-tested |
| evaluation layers are independent and stay `NotRun` without current evidence | `evaluation::summarize` | example-tested |
| no single corrupted byte or truncation of an Ogg Opus stream is accepted | strict framing with CRC and granule checks | exhaustively tested over a valid stream, property-tested |
| Ogg Opus facts and the profile envelope follow the server's rules; a file Forge refuses is not one the API would admit | `ogg_opus`, `audio_profile`, `checks` | differentially tested against the intake contract's table and an independent CRC |
| an export names every reason it is refused: rights, stale inputs, missing, stale or failed check | `handoff::plan_export` | example-tested |
| a check for one profile never opens or closes another profile's gate | gate filtered by check name | example-tested |
| exported bytes are re-read from the workspace, re-hashed and re-measured | `prepare_export` | example-tested, tamper-tested on disk |
| a bundle is never merged into or written over an existing directory; the manifest is written last | `adapters::bundle::write` | example-tested |

These guarantees are local. They say nothing about listening quality, loudness, rights, server
admission or publication.

## Command-line contract

Non-interactive; Forge never prompts. Results go to stdout, diagnostics to stderr. With
`--json`, stdout carries one envelope per invocation, with keys in alphabetical order:

```json
{"command":"capabilities","data":{"…":"…"},"outcome":"ok","schema":"cantos-forge.cli/v1"}
```

`command` is the full command (`artifact add`) or `null` for a usage error. Failures carry
`error` instead of `data`: `{"code","message"}` plus `action` (the command that resolves it,
when there is one) and `details`.

| Outcome | Meaning | Exit |
| --- | --- | --- |
| `ok` | did what was asked, including a safe no-op replay | 0 |
| `check_failed` | `validate` ran and recorded a failing result; `data` has the measurements | 1 |
| `refused` | a rule said no and nothing was written (`unknown_artifact`, `profile_required`, `export_refused`, …) | 1 |
| `blocked` | an operator action is needed first; `action` names it (`workspace_missing`) | 1 |
| `failed` | the environment or a stored record cannot be trusted (`workspace_corrupt`, `io`) | 1 |
| `usage_error` | invalid arguments; nothing ran | 2 |

`--help` and `--version` exit 0. `handoff export` refusals list every reason together in
`error.details`, each with its `code` and the command that fixes it, for example
`check_not_run` → `cantos-forge validate …`.

### The handoff bundle

`bundle.json` (`cantos-forge.handoff-bundle/v1`) holds the artifact and revision, the audio
file's digest and length, the profile, the profile check **re-run on the exact bytes at export**,
the producer-declared provenance and rights claim, the pinned inputs, the three evaluation
layers (an AI or human layer that was not run says `not_run`) and
`"submission": {"status": "not_run"}`. It is producer-declared and producer-measured only: not
server-inspected, not listened to unless a human layer says so, not approved or published.

## Build and verify

```sh
cargo xtask build cantos-forge          # cargo build --locked -p cantos-forge --bins
cargo xtask verify rust                 # workspace fmt, clippy and tests, including Forge
cargo test -p cantos-forge --locked     # Forge only
```

xtask builds Forge but never runs it inside build, test, preflight or CI, and Forge never
starts xtask. Running the binary directly (`target/debug/cantos-forge capabilities --json`) is
local use, not a repository check. The binary-level test in `tests/cli.rs` runs the built
executable on a temporary directory: that is a test of Forge, not xtask running it.
