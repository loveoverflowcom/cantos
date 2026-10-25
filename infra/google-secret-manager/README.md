# Google Secret Manager inventory

[`inventory.toml`](inventory.toml) is the reviewed, **nonsecret** list of Cantos secrets in
Google Cloud project `cantos-a348b`: secret ID, environment (`dev`/`prod`/`ops`), consumer,
the env or file contract it feeds, payload format and, once reviewed, the numeric version a
runtime binding pins. It never holds a value, a hash, a service-account key or a private path.

`cargo xtask secrets inventory` validates it offline; `status` and `import` refuse any secret or
project that is not listed. Adding a secret means adding its entry here in a reviewed change,
derived from the consumer's actual source contract.

The [operator runbook](../../docs/deployment/google-secret-manager.md) owns IAM, import,
pinning, rotation, rollback and runtime delivery; the [xtask contract](../../xtask/README.md#secrets)
owns the command.
