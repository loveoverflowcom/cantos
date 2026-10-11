# Kanidm identity provider (configuration only)

[Kanidm](https://kanidm.com) is the **proposed** identity and single sign-on provider, so Cantos
would not run its own account system. The choice is not yet a recorded decision. This directory holds **deployment documentation and an example server
configuration**; it contains no code and Cantos does not reimplement any Kanidm behavior.

## Status

| Claim | State |
| --- | --- |
| Example `server.toml` parses with Kanidm 1.10.5 (`kanidmd configtest`) | verified once, in a throwaway container with no network and a self-signed certificate |
| A Kanidm instance serving Cantos users | **not deployed** |
| `cantos-api` validating Kanidm-issued tokens (OIDC/OAuth2) | **not implemented**; the API still authenticates operator-issued development tokens |
| Roles, groups, OAuth2 clients, redirect URIs, MFA/passkey policy | **not designed** |
| Production TLS, backup, upgrade and recovery | **not decided** |

[ADR 0009](../../docs/decisions/0009-monorepo-layout.md) fixes the location only. Choosing Kanidm
as the identity provider and integrating it need their own work item and ADR.

## Intended boundary

- Kanidm owns credentials, sessions and identity lifecycle. Cantos stores no passwords.
- `services/cantos-api` remains the authority for what an authenticated identity may do:
  script ownership, rights, approvals and publication. Permissions are enforced in the backend,
  never in a client.
- Studio and Theatre are separate OAuth2 clients with separate scopes, so a listener session
  cannot open Studio routes.
- Kanidm configuration and deployment secrets stay out of Git. [`.env.example`](../../.env.example)
  lists nothing for Kanidm yet because no consumer exists.

## Check the example configuration

Use a Kanidm release you have chosen to run. The test below used `kanidm/server:1.10.5`. Generate
a disposable self-signed certificate; do not reuse it anywhere else.

```sh
mkdir kanidm-check && cd kanidm-check
openssl req -x509 -newkey rsa:2048 -nodes -keyout key.pem -out chain.pem -days 1 \
  -subj "/CN=idm.cantos.localhost" -addext "subjectAltName=DNS:idm.cantos.localhost"
chmod 644 key.pem chain.pem
cp ../services/kanidm/server.toml.example server.toml
podman run --rm --network=none -v "$PWD:/data:Z" docker.io/kanidm/server:1.10.5 \
  kanidmd configtest -c /data/server.toml
```

Success prints `This config rocks!`. A fresh data directory also logged a SQLite
`no such table: id2entry` error line before that; the command still reported success, and the cause
was not investigated. Config key names and versions change between Kanidm
releases; re-run this check when upgrading. It proves the file parses, not that the server is
reachable, correctly secured or integrated.
