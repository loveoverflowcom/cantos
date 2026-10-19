# Cloudflare Web rollout boundary

Cloudflare is the requested static Web destination in
[#278](https://github.com/loveoverflowcom/cantos/issues/278) and
[#280](https://github.com/loveoverflowcom/cantos/issues/280). This directory currently holds
documentation only: no allocated account/project mapping, static policy, Wrangler config,
publisher, Worker or Pages Functions runtime is implemented here.

The [rollout runbook](../../docs/deployment/cloudflare-rollout.md) owns inventory, qualification,
cutover, three recovery boundaries and Hosting-only retirement. #278 owns the provider decision,
tracked static policy and artifact-consuming publisher; #277 owns release artifacts/promotion.
Read their implemented contracts before execution. Do not create placeholder resources or a
second publisher in this directory. `cargo xtask hosting-report` reads an operator-declared
local record only and never publishes or queries a provider.

Theatre and Studio must remain separate static projects. API/PostgreSQL/Kanidm stay on the VPS;
Firebase Auth/email proof, Forge Functions/Gemini, App Distribution and media/drafts retain
their existing owners. Existing Firebase Hosting recovery assets remain until the operator
accepts production/recovery and ends the retention window. This source directory changes no
account, DNS record, production setting or cloud resource.
