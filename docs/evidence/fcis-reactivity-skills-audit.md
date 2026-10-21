# Functional core and Leptos reactivity skills audit

Date: 2026-10-21. Skills/guidelines only; no runtime source, test, dependency, schema, toolchain
or contract change. Baseline: fetched `origin/refactor/cantos-native-turso`, first audited at
`3a7ae29a4927eb58b38c030c1c5cf9bdf2808585`, then rebased onto
`d02b2dc8570825f310cc3b299d0c0cb619c88445` (merge of #355, which changed docs/skills plus two
contract/README files and none of the cited source). Isolated branch
`docs/skills-fcis-leptos-reactivity`; other worktrees and the original checkout were preserved.

Pinned versions read from the manifests and `Cargo.lock`: `leptos =0.7.8`, `reactive_graph 0.1.8`,
`tachys 0.1.9`. `reactive_stores 0.1.8` is a transitive dependency of `tachys`, not a direct
dependency of any Cantos crate. API statements in the changed skills were checked against those
crate sources, not against current upstream documentation.

## Invariant and owners

An agent reading the skills must keep business decisions deterministic and checkable, close
mutation authority to typed intents, execute I/O in a shell with ownership and commit semantics,
publish only reactive changes that happened, and preserve the Native Identity/Server/Turso and
Web cookie/native Bearer boundaries. Failure mode: a rule that equates functional style with
"no `mut`", a memo with O(1), or a registered harness with an executed proof.

| Changed owner | What it now owns |
|---|---|
| [functional-core](../../.agents/skills/cantos-engineering/references/functional-core.md) | purity by responsibility, ID/time injection shapes, transition semantics, closed mutation boundary, shell order and effect owners |
| [immutability](../../.agents/skills/cantos-engineering/references/immutability.md) | control state versus bulk data, conditional structural sharing |
| [typed errors](../../.agents/skills/cantos-engineering/references/typed-errors-and-composition.md) | readability carve-out for guards, `let … else` and monad naming |
| [mutation and formal](../../.agents/skills/cantos-engineering/references/mutation-and-formal.md) | control-kernel selection, abstraction mapping, auth/session/sync proof obligations |
| [clean code](../../.agents/skills/cantos-engineering/references/clean-code.md) | role-suffix naming and a non-performed rename proposal |
| [Web architecture](../../.agents/skills/cantos-leptos-web/references/web-architecture.md) | reactive ownership/publication against pinned APIs, keyed dynamic rows |
| [Web performance](../../.agents/skills/cantos-leptos-web/references/performance-and-caching.md) | keyed-row cost decomposition |
| [Web testing](../../.agents/skills/cantos-leptos-web/references/web-testing.md) | reactive publication and keyed-row regressions |
| Engineering, Leptos and code-review `SKILL.md` | routing only; criteria stay in the references |

Unchanged after review because they already cover this scope: `decoupling.md`,
`verification-strategy.md`, `persistence.md`, `identity-and-session-boundaries.md`,
`service-runtime.md` and `leptos-async.md`.

## Audit matrix

Status: **adequate**, **missing**, **contradictory** or **outdated** relative to the task.
Source lines are at `d02b2dc8`.

| # | Owner / section | Prior rule | Status | Source example | Failure mode | Change |
|---|---|---|---|---|---|---|
| 1 | functional-core, scope and shape | core never reads clock/IDs; shell owns them | missing: purity by folder vs responsibility; how IDs enter | `account_view.rs:179` `finish_session` calls `generate_uuid_string()` (window crypto or global counter); positive: `bearer/core.rs:89` `rotate(…, now, …)`, `account_view.rs:381` injected `next_operation_id` | nondeterministic transition documented as pure | new "Purity follows responsibility" subsection |
| 2 | functional-core, transactional rejection | `Err` leaves no partial mutation | missing: no-op vs effects vs replace; by-value ownership; event vs command | `account.rs:122` `leave` clears scope/projection before checked increment may return `Exhausted`; `account_view.rs:268-270` sets `cleanup_intent` before the fallible call | partial mutation on exhaustion, or an implicit fail-closed rule no contract states | "Transition semantics" section |
| 3 | functional-core / immutability | no `pub` fields; mutate through methods | missing: open closure accessors | `account_view.rs:214` `with_sync_mut`, `:232` `with_fenced_sync_mut`; callers in `live/runtime.rs`, `features/library/live.rs`, `live_player.rs`, `public_work.rs` | any consumer can break sync/projection invariants; owner only reconciles afterwards | "Close the mutation boundary" section |
| 4 | functional-core, shell | shell owns I/O; links to persistence | missing: execution order, domain `Effect` vs `leptos::Effect`, guard across await | positive: `live/runtime.rs` `persist_sync_journal` persists before mutation send | lock/borrow held over RPC; effects interpreted twice | shell-order section linking persistence/identity owners |
| 5 | immutability, value semantics | `Arc<[T]>` after measurement | missing: make-mut copy, O(1) clone ≠ O(1) update, control vs bulk | Theatre `state.get()` of whole account; Studio row-level `document.get()` | clone-heavy reads; Kani state includes bulk data | two subsections |
| 6 | typed errors ERR-02 | `?`/`and_then`, accumulation, no lossy `.ok()` | adequate; small readability gap | none | over-applied "monadic" style | one bullet; no monad vocabulary added as a requirement |
| 7 | mutation-and-formal, BMC | harness obligations and actual execution | missing: kernel selection, abstraction mapping, auth/session list, purge ≠ purged | `live/request.rs` harnesses unregistered/NOT_RUN; registered identity kernels | proof of a re-model or of a purge request claimed as shell behavior | "Selecting a control kernel" |
| 8 | clean-code, naming | verbs/domain nouns | missing: role suffixes | `account_view.rs` / `AccountViewState` is a session/sync machine | readers expect rendering code | suffix table plus proposal, no rename |
| 9 | Web performance, reactive CPU | `.with` vs `.get`, memo cost, untracked caution | adequate for reads; missing read-only handles and notification semantics | `features/account.rs:145-154` `state.get().phase()` on `RwSignal<AccountViewState>`; `live/runtime.rs:113` unconditional `try_update`; positive `view/live_shell.rs:47` fence memo, `public_work.rs` `Memo::new(… state.with(shape))` | whole-aggregate clone per label; notifications for refused/unchanged transitions | "Reactive state ownership and publication" |
| 10 | Web performance / script editor | key rows by stable identity (editor-specific detail) | missing for general dynamic lists; same-key stale rows; key with content | `features/library/live.rs:495` rows rebuilt inside one `state.with`; `public_work.rs:521` key includes `unavailable`; positive Studio `authoring_view.rs:347,355` keyed by ID | lost focus/IME; stale row text; remount on availability change | "Dynamic lists and keyed rows" |
| 11 | Web performance | memo/keyed cost cautions | missing cost decomposition | Studio `authoring_view.rs` per-row `document.get()` + linear `find` | unmeasured O(1) claims | "Keyed rows are not automatically O(1)" |
| 12 | Web testing | stale-response regression | adequate for async; missing publication/keyed-row/logout cases | none | regressions only checked by inspection | "Reactive publication regressions" |
| 13 | Leptos `SKILL.md`, immutability bullet | "reducers take state by value and return new state" | contradictory with borrowed-input transitions; silent on rejection ownership | n/a | by-value API that loses state on `Err` | bullet rewritten; one routing row added |
| 14 | code-review pass table | no route for transitions or reactive writes | missing routing | n/a | reviewers skip owner criteria | two rows citing owners; no criteria restated |
| 15 | Web architecture intro, §7, §9 | Theatre Web described as a skeleton at `3a7ae29a` | outdated, then resolved | Theatre has discovery, account/Library and isolated mock | wrong repository reality | resolved by #355 before this change; no edit here |
| 16 | Theatre Web README module table | `account.rs`, `account_view.rs`: "Pure session generation …" | contradictory with source row 1 | `account_view.rs:179` | README overstates purity | not edited; resolve by the runtime follow-up injecting the ID, then keep the README claim |

Not treated as gaps: the absence of the words "monad", "Writer" or "State" where semantics were
already stated; existing `&mut self` owner methods; Leptos `.update()` closures in the shell.

## Runtime follow-up checklist

None of these is implemented by this change. Each needs its own scoped change, tests and
evidence. Paths are current names; a rename would be its own step.

### Core and state ownership

- [ ] `AccountViewState::finish_session` receives the sync instance/operation ID as a fact or a
      shell generator; a plain `#[test]` asserts the exact value.
- [ ] Exhaustion in `AccountState::leave`/`begin_logout` either computes the next generation first
      or the owning contract states fail-closed clearing; a test asserts the full state on `Err`.
- [ ] `with_sync_mut`/`with_fenced_sync_mut` replaced by typed sync intents on the owner, each
      returning its outcome and whether the aggregate changed.
- [ ] Optional rename (`account_view.rs` → `account_machine.rs`, `AccountViewState` →
      `AccountSessionState`) in one change with imports and README updated.

### Kani obligations

- [ ] Decide whether Theatre account generation/fence admission becomes a registered kernel;
      if so, exact production symbols, symbolic generations, covers for stale/future/current,
      unwinding and an actual `cargo xtask verify kani` run on a supported host.
- [ ] Register or retire the two unregistered `live/request.rs` harnesses with cover goals.
- [ ] Any purge proof is paired with a kernel → aggregate → reactive publication test.

### Persistence and auth shell

- [ ] No signal borrow or storage handle held across `delete_session`, library or mutation awaits.
- [ ] Journal persistence failure never drops pending operations; unknown outcomes reconcile with
      the original operation ID.
- [ ] Web cookie and native Bearer transports keep their owning contract; no mixed-credential
      fallback; FE calls only the public Server API.

### Reactive reads and publication

- [ ] Readers (`features/account.rs`, `features/auth.rs`, Library sections) receive read-only
      handles or memoized projections instead of `RwSignal<AccountViewState>`.
- [ ] `state.get().phase()`-style reads replaced by borrowed or memoized projections.
- [ ] `live/runtime.rs` writes use `try_maybe_update` with the flag derived from the transition
      outcome; refused/stale/unchanged completions do not notify.

### Keyed rows, focus and IME

- [ ] Library sections render membership through keyed `<For>` by stable entity identity; rows
      read their entity reactively; removed rows are handled without panic.
- [ ] Decide whether `unavailable` belongs in the public-work unit key; test the intended
      remount/no-remount behavior and same-key content updates.
- [ ] Focus and Vietnamese composition survive sibling updates (`DOM-tested`), plus a recorded
      IME walkthrough where claimed.
- [ ] Studio authoring row titles: measure before replacing `document.get()` + `find` with an
      indexed projection.

### Logout and account isolation

- [ ] Logout/account switch publishes the purge with the phase change; no frame shows prior rows.
- [ ] Late completions for the previous fence leave state and notification counters unchanged.
- [ ] Journal/storage results from the previous account cannot block or overwrite the next one
      (existing `old_account_storage_failures_cannot_block_a_new_account_or_overwrite_its_journal`
      covers the core; the reactive path needs its own test).

## Evidence boundary

This record and the skills it cites are `documented`. No Web behavior, browser, database,
Identity RPC or Kani harness was exercised. Repository hygiene checks for the change are
reported in its pull request.
