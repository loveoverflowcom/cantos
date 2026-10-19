# Feature: [short name]

- Status: [draft / ready / in progress / accepted]
- Owner: [person or team]
- Issue: [link]
- Related decisions: [links or N/A]

## User outcome

[Describe the creator or listener task, current problem, and intended result.]

## Scope

In scope:

- [behavior]

Out of scope:

- [excluded behavior]

## Flow and states

[Describe the main flow and relevant loading, empty, error, cancellation, retry, and recovery states. Include native playback behavior when applicable.]

## Contracts and dependencies

- Prerequisites: [issues or decisions]
- Domain/API/Script IR changes: [changes or N/A]
- Version and migration behavior: [behavior or N/A]
- Shared Web/CMP design tokens or components: [impact or N/A]
- Provider/storage dependencies: [dependencies or N/A]
- Data sources: [real API/artifacts, explicit mock/fixture, or unavailable; exact contract and revision]
- Integration boundary: [implemented consumer/adapter versus proposed work; remaining live acceptance]

## Production impact

- Affected dialogue, scene, or episode revisions: [scope or N/A]
- External-producer reuse/regeneration and stale-review impact: [rules or N/A; no API synthesis implied]
- Retry, resume, and duplicate-prevention behavior: [rules or N/A]
- Cost budget or limits: [approved budget, owner, and evidence or N/A]
- Permission and publication approval impact: [impact or N/A]

## Acceptance criteria

- [ ] [observable result]
- [ ] [relevant failure/recovery result]

## Verification

| Criterion | Check or evidence | Result |
| --- | --- | --- |
| [criterion] | [test, screenshot, preview, or measurement] | [pending / passed / failed] |

Bind evidence to the checked source/revision, environment, exact inputs and observed scope.
Separate fixture/structural checks from real API/provider/runtime, human listening and
publication acceptance. An accepted decision or a closed issue is not implementation evidence.

## Delivery and open questions

[List reviewable implementation steps, rollout or recovery needs, unresolved decisions, and linked follow-up work.]
