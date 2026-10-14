# Locked development-only Quint toolchain

The repository surface is `cargo xtask verify behavior [--capability ID] [--setup] [--conformance]`.
Do not add a competing npm script or invoke a solver implicitly.

Pins are Node **24.19.0**, npm **11.9.0** and official `@informalsystems/quint` **0.33.0**.
`package-lock.json` pins every transitive version, official registry origin and SHA-512
integrity. `packageManager`, engines, `.node-version` and the installed package set must
agree. The upstream npm package declares Node >=18; this repository narrows the tested
contract to its exact current Node LTS toolchain. Tool setup is explicit and local to this
directory; it never changes global tool defaults or production dependencies.

`--setup` runs only `npm ci --ignore-scripts --no-audit --no-fund` against the official
registry, with an ignored local npm cache and isolated empty user/global npm config.
No lifecycle script, credential setup, evaluator binary or solver download is permitted.
Successful setup records a local checksum receipt under ignored `target/verification/behavior/`.
Installed-tree, CLI, executable and configuration hashes must match that receipt before
Quint executes; missing/stale receipts require explicit setup. This detects local changes,
not hostile replacement of both tool and receipt: npm integrity, the installed Node/npm
runtime and the local filesystem remain trusted prerequisites.
Normal verification uses the installed CLI through the pinned Node process and explicitly
selects `--backend=typescript`; 0.33.0's default is Rust. Ambient NODE_PATH is cleared to
prevent undeclared module fallback; nonempty NODE_OPTIONS is refused. Versions and source/
installed-tree/executable hashes are recorded, and checked again before a model PASS.

The version was checked against the [official release](https://github.com/quint-co/quint/releases/tag/v0.33.0)
and official npm metadata. It fixes action/effect regressions and qualified import behavior
relative to 0.32.0. The [CLI manual](https://quint.sh/docs/quint) distinguishes `run` sampling
from `verify` solver checking; this pilot never invokes `verify`.

Upgrade the pins/lock deliberately. Rerun the output-contract unit tests, named runs,
property campaign, witnesses and every negative control before accepting new evidence.
Repository installation capability, model execution, implementation conformance and real
infrastructure/device acceptance are different claims.

`--conformance` adds the registered test-only Rust pure-core observer with locked/offline
Cargo, no new dependency or provider process. Compiler override environments and ambient
observer configuration are refused. Each owned observer process has a 180-second bound;
Quint queries retain their original catalog bounds. Model and implementation artifacts have
separate statuses. See the [observation protocol](../../specs/behavior/observation-protocol.md).
