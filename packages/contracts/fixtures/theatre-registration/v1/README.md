# Theatre registration v1 fixtures

Synthetic shape-only request corpus shared by JSON Schema and Rust DTO tests. The
repeated `a` proof is fixture data. Domain and HTTP tests separately enforce email
syntax, proof hash/expiry, same-origin admission and single-use effects. These
values do not establish an account, provider permission, session or mail delivery.

Firebase confirmation fixtures separately admit only operation ID and action code. Private setup
responses are checked against `$defs.firebase_confirmation`; the legacy public status root is
unchanged. All addresses, operation IDs, action codes and setup URLs are inert fixtures.
