# Discovery/Playback → Library/Progress

The [Theatre contract](../../packages/contracts/theatre-discovery-v1.md) establishes stable Work,
ContentUnit and immutable publication/rendition/checksum identity. It does not establish a
listener account, library, progress, history, bookmark, offline grant or production media URL.

## Dependencies for the next separately scoped PR

1. Fetch fresh develop and the exact published Discovery head; inspect whether Publication
   PR91 and private Storage PR90 have actually merged. Stack on published prerequisite commits
   if needed, preserving other branches and distinguishing inherited work. Do not merge into
   develop without separate authority. No CI wait is required for local delivery.
2. Coordinate with the parallel Identity campaign: Discovery reserves migration 0017
   `0017_listener_discovery.sql`; Identity must use 0018 or higher. Library must consume the
   reviewed typed listener identity adapter, not the Studio cookie, legacy raw development
   token or a duplicated account registry. Anonymous public discovery/playback eligibility
   remains distinct from account-owned saves/progress.
3. Use WorkId for saved works, ContentUnitId for continuity, PublicationId for the exact media
   snapshot, and rendition/SHA/duration for timing identity. Choose unit-following versus
   release-pinned progress explicitly. New release bytes never silently substitute in a player.
4. Removal of playable access must not delete user progress/history. Discovery only resolves
   active releases anonymously; retained/superseded access needs an account-scoped retention
   decision, not a weakened anonymous resolver. Rights expiry/revocation and withdrawal always
   remove playable access, including after an issued grant.
5. Add account-scoped conditional versions, operation IDs and identical-intent replay semantics
   for durable writes, with multi-device conflict policy. Issuing a grant or visiting a page
   never records that somebody listened. No native engine or audio generation belongs here.
6. Continue L1→L2→L3, property/proof production kernels, before application/SQL/HTTP. Run the
   required xtask gates; separate actual FAIL from unavailable/NOT_RUN acceptance. Do not bypass
   the denied disposable PG socket route or suppress managed private-input fixture failures.

## Required inherited acceptance

Migration 0017 and 0016 replay/app-role grants; real visibility and copied-scalar-corruption SQL
regressions; current representative metadata and Unicode search; insertion/deletion/replacement
paging and published unit-order policy; withdrawal/rights revocation during signing/read expiry;
restart/reconnect/crash behavior. Use an authorized disposable PG environment and
`cargo xtask verify postgres`; startup currently fails at /tmp Unix-socket creation EPERM.

No measured p95/query plan, DBSP adoption, real public-object integrity/Range/CORS/cache or
per-fetch revocation evidence exists. Default playback delivery is unavailable; private storage
cannot be repurposed as a public origin. Operator-owned live storage/public promotion and legal
rights/subjective listening acceptance remain separate.

## Web/CMP adoption

Web currently has three enum WorkIds and static mock text. CMP DemoMapping and PlaybackSource
admit only three sample keys. Both root live probes deliberately reject success instead of
parsing these resources. They need actual dynamic identity and drift-checked DTO adapters before
live adoption, followed by real media/native acknowledgement, interruption, keyboard/large-text,
accessibility and restore/concurrency checks. Keep the isolated mocks intact until that work.
