# Kanidm opt-in evaluation runtime

Kanidm remains a **proposed identity provider**, with no Cantos OIDC integration, account/group
mapping, client registration or production deployment. The API retains its development sessions;
this infrastructure change grants no Theatre/Admin authentication capability.
[ADR 0021](../../docs/decisions/0021-podman-service-runtime.md) standardizes the container runtime,
not the identity decision. [ADR 0009](../../docs/decisions/0009-monorepo-layout.md) fixes its location.

The [manifest](../manifest.toml) pins official `docker.io/kanidm/server:1.10.5`, matching the
previous example's chosen release. A historical throwaway `configtest` was reported for that
release; it is not fresh runtime evidence for this PR. The current layout separates read-only
`/config` from persistent `/data`. No Rust crate or custom Containerfile is needed.

## Operator prerequisites

Use an already installed rootless Podman connection (Linux or running macOS Podman Machine).
Prepare a private directory **outside Git**, caller-owned mode0700, containing only the intended
`server.toml`, `chain.pem` and `key.pem` (regular caller-owned files, no group/other permissions).
xtask never creates certificates, recovers admin accounts or changes trust/security settings.
On macOS the directory must already be shared with the Machine. Export the
[host environment example](env.example); xtask does not load dotenv files.

Copy [server.toml.example](server.toml.example) and set domain/origin deliberately before first
start. Origin must use HTTPS, match the domain and chosen host port. Keep the fixed internal
bind, database and TLS paths. This small evaluation contract rejects extra configuration keys;
production configuration needs its own reviewed runtime rather than this command.

Supply a valid leaf certificate plus issuing chain in `chain.pem` and its matching private key.
For this local health contract the leaf needs both the chosen identity hostname and
`localhost` in its DNS SANs. The pinned `kanidmd scripting healthcheck --verify-tls` connects
to `localhost:8443`; upstream loads issuing certificates after the leaf as trust roots.
A lone self-signed leaf is insufficient for that verified health probe. Certificate issuance,
trust installation and key permissions are operator work, not an implicit task action.
Never commit key/cert contents or identity data.

## Explicit lifecycle and validation

```sh
export CANTOS_KANIDM_CONFIG_DIR=/absolute/private/cantos/kanidm
export CANTOS_KANIDM_PORT=8443
cargo xtask services up kanidm
cargo xtask services status
cargo xtask services logs kanidm
cargo xtask services down kanidm
cargo xtask services up kanidm
```

Commands become available in the lifecycle commit. `up --all` excludes optional Kanidm;
`down --all` may stop an already-owned Kanidm container. Startup first statically validates the
private configuration, then runs `/sbin/kanidmd configtest -c /config/server.toml` in a newly
created labeled, network-disabled throwaway container with tmpfs `/data`. It never validates
against an existing identity database. `configtest` may initialize temporary data; its success
proves upstream configuration acceptance, not TLS serving, identity integration or backup safety.

The actual server uses `/sbin/kanidmd server -c /config/server.toml`, a named
`cantos-<instance>-kanidm-data` volume, and a host mapping only on `127.0.0.1`. Health uses the
pinned scripting healthcheck with TLS verification. Upstream1.10.5 establishes an HTTPS
connection to `/status` but does not check the returned status/body; report that bounded
readiness, never end-to-end identity acceptance. Podman bounds each health probe and xtask
bounds startup. Failed startup preserves the server container/volume for explicit inspection.

`down` preserves container/volume. Restart refuses image/port/config drift; it never changes an
existing domain, silently upgrades data or deletes identity state. Server initialization may run
Kanidm's own first-start internal migrations on **its new volume**; Cantos never calls domain
rename, raise, remigrate, vacuum or restore. Backups, safe upgrades, MFA/passkeys, Studio/Theatre
OAuth2 scopes and production recovery need a separate ADR/work item.

Sources: [upstream configuration](https://kanidm.github.io/kanidm/stable/server_configuration.html),
[pinned CLI options](https://github.com/kanidm/kanidm/blob/v1.10.5/server/daemon/src/opt.rs),
[pinned health implementation](https://github.com/kanidm/kanidm/blob/v1.10.5/server/daemon/src/main.rs).
