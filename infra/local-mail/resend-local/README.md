# Synthetic Resend local mail fixture

This is an opt-in **HTTP mail capture**, not an Identity verification provider. Its adapter in
`xtask/src/resend_local.rs` accepts one compile-time message addressed to `cantos.invalid`.
It has no arbitrary recipient/body/key argument and implements neither Identity's `Mailer`
nor `VerificationActions`. A successful capture proves no mailbox ownership, account or session.

## Reviewed software

| Input | Pin / provenance |
| --- | --- |
| Third-party package | [`resend-local@0.0.7`](https://www.npmjs.com/package/resend-local/v/0.0.7), y-hiraoka, MIT; not official Resend software |
| Source | [`41e474ee644049ed75a037c56d0c12c21f938c5d`](https://github.com/y-hiraoka/resend-local/tree/41e474ee644049ed75a037c56d0c12c21f938c5d), published 21 September 2025 |
| npm archive | SHA-1 `4e7df9c08b0d0c8185004e417e5544a0f35a6c75`; npm SHA-512 integrity is recorded in `release-files.json` and `package-lock.json` |
| Package bytes | All 133 release files are checked against `release-files.json` before process startup; drift, nonregular files and escaping paths refuse |
| Runtime | Installed Node.js 22+; exact dependency graph in `package-lock.json` |
| Security override | Next.js **15.5.28**, not upstream's vulnerable 15.5.2; the [official maintenance security update](https://nextjs.org/blog/nextjs-security-update-october-14-2026) and [RSC advisory](https://nextjs.org/blog/CVE-2025-66478) motivate this isolated override |

The archive hashes validate package integrity. npm provenance links the release digest to the
source commit; `npm audit signatures` verifies registry signatures and available attestations
for the installed graph. The Next override is a Cantos
fixture dependency choice, not an upstream resend-local release or a production qualification.
No provider credential is required. `re_cantos_synthetic_fixture` is a public, nonfunctional marker;
the upstream middleware checks only its prefix.

## Explicit preparation and run

Preparation is a separate operator action after approval of this exact third-party package.
Use a clean environment with no npm/provider credentials. From the repository root:

```sh
npm ci --prefix infra/local-mail/resend-local --ignore-scripts --no-audit --no-fund
npm audit signatures --prefix infra/local-mail/resend-local
cargo xtask run resend-local
```

`npm ci` installs the reviewed lock; no lifecycle scripts, global install or `npx` is needed.
`xtask` never installs, upgrades, downloads or falls back to a cloud service. By default it uses
this directory's installed `node_modules/resend-local`. An alternate prepared installation may
be selected with absolute `CANTOS_RESEND_LOCAL_PACKAGE`; it must pass the same byte/version checks
and have the patched sibling `next` installation. Do not invoke the upstream starter directly.

The one-shot command starts the fixture, posts `synthetic-email.json`, retrieves and compares the
capture, then stops its own process group. It reports **simulated** delivery. A changed/invalid
acknowledgement is uncertain, never proof of non-delivery; the write is not retried. Redirects
are not followed. HTTP exchanges reuse the existing bounded readiness HTTP parser.

For the full local Theatre composition, select the lane explicitly:

```sh
cargo xtask run cantos-theatre --web --local-mail resend-local
```

This starts the same synthetic fixture alongside Server/Identity/Web and leaves **Identity mail
disabled**. Signup verification through this fixture is unavailable. The ordinary run default is
also mail-disabled; `--local-mail firebase` explicitly retains the existing native action/proof
lane. [The command contract](../../../xtask/README.md#run-and-app) owns orchestration.

## Runtime and data boundary

The launcher starts the verified server module directly, avoiding the upstream starter's
inherited environment and shared database path. It uses an empty child environment, an ephemeral
`127.0.0.1` listener, and a fresh private `artifacts/local-mail/<run-id>/mail.sqlite` copied from
the empty release template. Captures remain after shutdown; there is no cleanup command.
Existing Identity/Server DB/WAL, media and evidence are never opened or removed.

Before importing the package, startup negative controls reject outbound TCP, TLS, UDP and child
process creation. Only the fixed JSON POST and its mail-ID GET enter the server; browser Origins,
foreign Hosts, redirects, dashboard/image/RSC paths, action headers, upgrades and changed request
bodies are refused. Thus this lane cannot supply an attachment URL or functional verification
token to the third party. Provider stdout/stderr are suppressed, with a bounded private readiness
handshake and the existing process-group supervisor. These application guards are not an OS
sandbox or a complete audit of native dependencies.

The upstream fixture stores plaintext HTML/text/recipients and hardcodes `delivered`. Its HTTP
API is not SMTP and does not emulate Firebase UID/custom-token/OOB-consumption or sealed-proof
receipts. Its unsupported API routes and delivery status cannot accept real Resend behavior.
Official [Resend test addresses](https://resend.com/docs/dashboard/emails/send-test-emails) still
use the live account/API and quota; no such call is made here.

## Verification and remaining work

`cargo test --locked -p xtask` includes the HTTP adapter, request/body/outbound guards, package
drift checks, CLI selection and production-default planning regressions. The guard tests need
Node.js 22+ but no installed emulator. Ordinary tests/builds/preflight never start the package.
The explicit `cargo xtask run resend-local` is the installed-package boundary check.

Identity's existing `lettre` SMTP/native fragment proof and Firebase action ports remain intact.
Current CMP native-first completion accepts Firebase action links only. Sending usable native
proofs through a plaintext mailbox needs a reviewed transient-storage design and explicit test
scope; replacing native Firebase actions also needs a protocol decision, client changes and new
acceptance. This slice does neither. Password reset remains outside the implemented contract.
See [ADR 0049](../../../docs/decisions/0049-synthetic-local-mail.md).
