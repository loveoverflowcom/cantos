# Identity RPC and credential dependency qualification (#334 / #332 prerequisite)

Reviewed 2026-10-21 for the `cantos-identity` `service` feature and the `cantos-identity-rpc`
crate. Versions are exact pins (`=x.y.z`), chosen from the crates.io index and docs.rs of that day,
not from memory. This is a scoped manual review: not a security audit of any library and not a
workspace-wide advisory clearance.

| Crate | Pin | Why this version | Licence | MSRV | Features |
| --- | --- | --- | --- | --- | --- |
| `argon2` (RustCrypto) | `=0.5.3` | The long-lived stable 0.5 line. 0.6.0 (published 2026-08-27) is under two months old and moves to `password-hash` 0.6 whose 0.6.0 release was yanked; the project takes the matured line. | MIT OR Apache-2.0 | 1.65 | `default-features = false`, `alloc`, `password-hash`. No `rand`, no `parallel`: salts come from `getrandom` 0.3.4 (already pinned) |
| `password-hash` | `0.5.0` (transitive of argon2) | Resolved by argon2 0.5.3 | MIT OR Apache-2.0 | 1.60 | PHC string codec only |
| `tonic` | `=0.14.5` | Newest 0.14 release older than two weeks with MSRV 1.75 (0.14.6, 2026-05-07, requires 1.88) | MIT | 1.75 | `default-features = false`, `transport`, `codegen`, `router`. No TLS, gzip or reflection |
| `tonic-prost` | `=0.14.5` | Matches tonic | MIT | n/a | defaults off |
| `prost` | `=0.14.4` | The version tonic-prost 0.14 resolves; already in the lock for the Turso engine | Apache-2.0 | 1.85 | `derive`, `std` |
| `tonic-prost-build`, `tonic-build`, `protoc-bin-vendored` | `=0.14.5`, `=0.14.5`, `=3.2.0` | **Generator only**, in the isolated `codegen/` workspace with its own lockfile; never built by a normal build. protoc 3.2.0 is the version already used by the Studio protobuf oracle | MIT / Apache-2.0 | n/a | n/a |
| `lettre` | `=0.11.23` | Identical pin and features to `cantos-server` | MIT | n/a | `builder`, `smtp-transport`, `tokio1-rustls`, `ring`, `webpki-roots`, defaults off |
| `tokio`, `tokio-stream` | `=1.47.1`, `=0.1.19` | Already locked | MIT | n/a | `net`, `signal` added to the existing set |
| `zeroize` | `=1.9.1` | Already locked | MIT OR Apache-2.0 | 1.85 | `alloc` added (needed to zeroize a `String`) |
| `hyper-util`, `tower`, `prost`, `tonic-prost` | as above | **Dev-dependencies only**: the test UDS client | MIT / Apache-2.0 | n/a | n/a |

No new toolchain, no Turso pin change. `Cargo.lock` gains exactly `argon2`, `password-hash 0.5.0`,
`blake2`, `hyper-timeout`, `tonic`, `tonic-prost` and the workspace crate `cantos-identity-rpc`;
`prost 0.14.4`, `hyper`, `hyper-util`, `axum` and `tower` were already present.

## Parameter policy

Argon2id, version 0x13, `m = 19456 KiB`, `t = 2`, `p = 1`, 32-byte output, 16-byte salt: the
OWASP Password Storage Cheat Sheet minimum equivalent. A stored hash below those values is refused
by the verifier; above `m = 65536`, `t = 6`, `p = 4` it is refused **before** any allocation.

## Library vector

`credentials::hasher` reproduces the RFC 9106 §5.3 Argon2id vector (32×`01` password, 16×`02` salt,
8×`03` secret, 12×`04` associated data, `m=32, t=3, p=4`) with the tag copied from the RFC text. It
validates the pinned library, not the service parameters.

## Advisory scan

`cargo deny --manifest-path services/cantos-identity/Cargo.toml --all-features check advisories licenses`
on 2026-10-21 against the advisory database at its then-current `main` (fetched by the command,
revision not pinned; reproduce with a fresh database and record its revision):

* **Clean for every crate this work adds:** `argon2`, `password-hash 0.5.0`, `blake2`, `tonic`,
  `tonic-prost`, `hyper-timeout` and the prost/tokio/tower/hyper stack report no advisory and no
  yanked version. `cargo deny` flags the four workspace crates only for lacking a `license` field.
* **Not clean, and not introduced here:** the TLS chain behind `lettre` (the **same** pin and lock
  entry `cantos-server` already uses) resolves `rustls 0.23.45`
  (RUSTSEC-2026-0346, -0347, -0348, -0349), `rustls-webpki 0.103.15` (RUSTSEC-2026-0382) and
  `webpki-roots 1.0.9` (RUSTSEC-2026-0381). The identical versions are in the base branch lock.
  `cargo update` of those three entries is a lock-only change that also alters the Server's TLS
  stack, so it is left to a separate security PR rather than folded into this feature.
  **Consequence:** Identity's production relay TLS (`starttls`/`implicit`) must not be accepted for
  production until that bump lands (nothing in code blocks those modes; the README repeats this); the `loopback-plaintext` test path does not use TLS. This is a
  scoped scan, not a clearance of the workspace.

## Measurements

Release-build footprint and idle/peak RSS under concurrent hashing are **not measured** in this
change (marked `ACCEPTANCE-PENDING` in `server.rs`); the debug host used for tests is not a
footprint measurement.

## Sources

[argon2 0.5.3](https://docs.rs/argon2/0.5.3/argon2/), [RFC 9106](https://www.rfc-editor.org/rfc/rfc9106.html),
[OWASP password storage](https://cheatsheetseries.owasp.org/cheatsheets/Password_Storage_Cheat_Sheet.html),
[tonic 0.14.5](https://docs.rs/tonic/0.14.5/tonic/),
[lettre 0.11](https://docs.rs/lettre/0.11.23/lettre/), crates.io index and API metadata of 2026-10-21.
