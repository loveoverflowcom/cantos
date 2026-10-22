# Theatre development catalog seeding

`cargo xtask seed theatre [--reset]` is an explicit native development lane, never a gate or CI
prerequisite. It requires the Server's `catalog_http` fixture, currently absent while the business
port is incomplete, so planning refuses before setup. No seed success is claimed.

The intended lane uses private run-owned Server/media files and a development token with the
Server stopped for operator file access, then talks to the Server over HTTP. Identity owns its
separate file. Never substitute a persistent/user DB, reset Identity, restore PG services or
weaken the fixture/admission guard to make a demo work. `--environment` and `--test-cluster`
are retired refusals. Startup alone does not accept publication, playback, TLS or browser/native auth.

[Original PostgreSQL seeding record](https://github.com/loveoverflowcom/cantos/blob/b2611c3db3896ce9733280a319008924ee8bf974/docs/deployment/theatre-seeded-catalog.md) retains
synthetic dataset/journal/recovery provenance for porting. It is historical, not a runnable workflow.
