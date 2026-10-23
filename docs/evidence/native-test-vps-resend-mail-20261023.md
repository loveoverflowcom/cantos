# Native test-VPS mail through Resend SMTP (23 October 2026)

Source-bound observation of one **owner-authorized test** change on the host described in the
[native test-VPS cutover](native-test-vps-cutover-20261023.md). It follows that record's "mail
provider (`mail: missing`)" gap. It is not production acceptance, not a mailbox-delivery or
browser journey result and not a supported deployment lane (the Quadlet renderer is still #348 PR B).

## Why not Firebase

The Firebase provider needs the action link to open the Cantos handler so Identity can consume the
action code itself ([protocol](../architecture/firebase-verification.md#implemented-protocol-and-durable-boundaries)).
For project `cantos-a348b` (Blaze, Email/Password enabled, Identity Toolkit API enabled, `cantos.cc`
added to Authorized domains) saving the custom action URL `https://cantos.cc/` failed twice:
`PATCH /v2/projects/cantos-a348b/config?updateMask=notification.sendEmail.callbackUri` returned
HTTP 400 `EMAIL_TEMPLATE_UPDATE_NOT_ALLOWED` (`INVALID_ARGUMENT`) at 13:46:57Z and 13:47:26Z. The
default Firebase handler would consume the code outside Cantos, so no receipt could be produced.
The existing SMTP adapter was selected instead; no code changed.

## What was observed

| Layer | Result | Evidence and limit |
| --- | --- | --- |
| Identity configuration | PASS | `CANTOS_IDENTITY_MAIL_PROVIDER=smtp`, `SMTP_HOST=smtp.resend.com`, port `465`, `TLS=implicit`, username `resend`, sender `no-reply@mail.cantos.cc`; Identity readiness line `{"event":"ready","mail":"configured"}` |
| Image default | FINDING | the image sets `MAIL_PROVIDER=disabled`; SMTP settings with it exit `status=2` ("MAIL_PROVIDER is invalid"), so `smtp` must be explicit |
| Container network | FINDING | `Network=none` has no egress; `Network=pasta` fails on rootful Podman 5.4.2; `Network=cantos.network` started. A failed restart briefly stopped Identity and the dependent Server; both were active again and `https://cantos.cc/` returned 200 |
| Sending domain | PARTIAL | `mail.cantos.cc` added in Resend (Tokyo); TXT `resend._domainkey.mail` and CNAME `rsend.mail`, `send.mail` (DNS only) created in Cloudflare and resolved from 1.1.1.1; Resend status was `Pending` when observed |
| Mail delivery | NOT_RUN | no mail, DKIM alignment result or mailbox receipt was observed by the agent; the owner reported a later sign-up attempt, which is not independently verified here |

## Not done

Mailbox delivery and link-open evidence; Resend `Verified` status; the `rustls` advisory re-check
the Identity README requires before accepting `implicit`/`starttls`; DMARC and receiving MX records;
an activated native Quadlet (#348 PR B). The Resend API key was shared in chat and must be rotated;
the key is stored only in the host's `identity.env` (mode 0600, backup `identity.env.bak-20261023`)
and is not recorded here.
