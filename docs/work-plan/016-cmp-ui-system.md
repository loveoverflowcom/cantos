# CMP UI preview and consumer follow-ups

Active review inputs for [#143](https://github.com/loveoverflowcom/cantos/issues/143) and
[#145](https://github.com/loveoverflowcom/cantos/issues/145). The earlier three-session plan,
PR43 matrix and appendices are [archived](../archive/superseded-plans/016-cmp-ui-system.md).
GitHub owns execution status; old Mac/session permissions are not current instructions.

## Boundary

Use the current [CMP package](../../packages/ui-system/cmp/README.md), [component admission
contract](../../packages/ui-system/shared/README.md#preview-registration-and-adoption),
[mobile architecture](../architecture/mobile.md) and explicit `:preview-sandbox` ownership.
Tokens/components/previews already exist. Studio Mobile stays out of scope; Theatre demo
behavior is not production native audio or persistence.

## Recommended next boundaries

1. Finish one #143 component/state qualification against actual token/theme injection, long
   Vietnamese copy, both themes, large text and relevant adaptive layouts.
2. Finish #145's motion/gesture/reduced-motion interruption evidence using the existing injected
   policy and replay clock, then observe the relevant Android/iOS runtime.
3. Reconcile consumer adoption with qualified components and the production-safe `:shared`
   boundary. #122's extracted sandbox and production Android exclusion slice are completed;
   do not rebuild or move mock adapters back into `:shared`.
4. Keep #167's renderer-neutral localization decision separate. #169's locale persistence and
   comparison slice is completed; reuse its evidence and retain any named platform/runtime gaps.

## Exact next task for session 3

This heading is retained for existing package links. The old session-3 assignment is historical;
use the review boundaries above and the linked issue's current acceptance. There is no
automatic next session or instruction to rerun the old command campaign.

## Review and evidence

Use the current Compose-preview/Android runbooks and repository cadence. Distinguish common/JVM
tests, semantics/captured pixels, emulator/device observations and release-artifact exclusion.
A preview, mock snapshot or production-host build does not prove native playback, durable
offline data, TalkBack/VoiceOver or real account integration. Record commands in the PR/evidence
without appending another session diary here.
