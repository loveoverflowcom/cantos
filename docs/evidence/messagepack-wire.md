# MessagePack wire evidence

Evidence for the optional MessagePack representation of the Studio revision routes
([architecture](../architecture/wire-codec.md), [ADR 0013](../decisions/0013-optional-messagepack-wire.md)).
Branch `feat/studio-messagepack-wire` from `develop` at `44886f1`, 2026-10-11.

Machine for every local number below: Intel Core Ultra 7 155H (22 logical CPUs), Linux, Rust
1.87.0, Trunk 0.21.14, Node 24.21.0, the Claude desktop in-app browser (Chrome 152.0.7977.130).
Each timing is one session on one machine: a median of 7 rounds with the min–max of those rounds,
no confidence interval. Sizes are exact.

## Claims ledger

| Claim | Evidence level | Evidence |
| --- | --- | --- |
| Rust MessagePack bytes equal an independent encoder's for every Studio fixture | differentially-tested | `msgpack_wire::independent_goldens_equal_rust_bytes_and_decode_to_the_json_fixture_values`; `scripts/test_reference_msgpack.py` |
| the independent oracle follows the MessagePack format boundaries | example-tested | `test_reference_msgpack.EncoderFollowsTheSpecification` (hand-written expected bytes); the same table in `msgpack_wire::integer_and_string_formats_follow_the_specification_boundaries` |
| every Studio and production fixture is the same DTO through JSON and MessagePack, including null, nested arrays and tagged enums | example-tested | `msgpack_wire::other_contract_fixtures_survive_json_to_msgpack_to_json_including_tagged_enums`, `…absent_and_null_optionals…` |
| maps are named; positional encoders are refused; unknown, missing, duplicate and mistyped fields are refused in both representations | example-tested | `msgpack_wire::{every_struct_level_is_a_map_keyed_by_field_name, key_order_is_free_but_positional_encoders_are_refused, unknown_missing_duplicate_and_mistyped_fields_are_refused_in_both_representations}` |
| hostile bodies are bounded: every strict prefix truncated, trailing bytes, invalid UTF-8, depth, 4 Gi length prefixes | example-tested (prefixes: bounded exhaustive) | `msgpack_wire::{every_strict_prefix…, bytes_after_one_complete_value…, invalid_utf8…, nesting_is_bounded…, declared_lengths_beyond_the_input…}` |
| Unicode text, including decomposed marks, is byte-identical through both representations | example-tested | `msgpack_wire::unicode_text_crosses_both_representations_byte_for_byte` |
| canonical export bytes and `sir-c1`/`sir-e1` digests survive MessagePack; a special-character draft admits identically | example-tested | `wire_contract::{canonical_export_bytes_and_both_digests_survive_the_msgpack_representation, a_draft_with_vietnamese_and_special_characters_admits_identically_from_either_representation}` |
| `Content-Type`/`Accept` select the codec per the architecture tables, with RFC 9110 qvalues and specificity | example-tested | `http::wire::tests::{content_type_selects_the_decoder…, accept_keeps_json_unless_msgpack_is_strictly_preferred, quality_values_follow_rfc_9110_syntax, repeated_accept_fields_form_one_list}` |
| `415`, `406` and malformed bodies never reach the handler and stay JSON errors; JSON responses are byte-identical to `axum::Json` | example-tested against a probe route | `http::wire::tests::{refused_representations_never_reach_the_handler_and_stay_json_errors, a_json_response_is_byte_identical_to_axum_json, every_request_and_response_codec_pair_carries_the_same_dto}` |
| a failed MessagePack encoding after a store call is the ambiguous `503 unavailable` | example-tested | `http::wire::tests::a_failed_msgpack_encoding_is_reported_as_an_ambiguous_outcome` |
| Origin and session refusals are the same JSON errors, with no-store/nosniff, for both codecs on all four routes | example-tested through the real router (lazy pool, no database) | `wire_contract::origin_and_session_refusals_are_the_same_json_errors_for_both_representations` |
| revision facts, authorization, stale and `operation_reused` decisions are identical; a lost MessagePack response replays in either codec without a second revision; refusals write nothing | integration-tested — **CI only**, not run locally | `revisions_postgres::{msgpack_and_json_carry_the_same_revision_facts_on_every_negotiated_route, a_lost_msgpack_response_replays_in_either_representation_without_a_second_revision, refused_representations_write_nothing_and_other_routes_stay_json_only, msgpack_requests_get_the_same_origin_session_and_permission_decisions}` |
| after a host kill and PostgreSQL restart, MessagePack head and save replay decoded by the independent oracle equal the JSON revision | fault-injected, integration-tested — **CI only** | `scripts/test_postgres.py` (`msgpack_http`); its request/response handling is example-tested with doubles in `test_postgres_runner` |
| the Studio decodes by response `Content-Type`, keeps errors JSON and treats an undecodable success as ambiguous; JSON request bytes equal the previous transport's | example-tested (host) | `cantos_studio::wire::tests` |
| the built WASM client sends and decodes MessagePack and JSON in a browser | interaction-tested against a stub backend | [Browser runs](#browser-runs) |

## Commands and results

All with `RUSTUP_TOOLCHAIN=1.87.0` from the repository root, on the uncommitted branch tree.

| Command | Result as printed |
| --- | --- |
| `cargo xtask verify repository` | `PASS` (repository checks; 117 Python tests incl. `test_reference_msgpack`, `test_postgres_runner`) |
| `cargo xtask verify contracts` | `PASS` script-ir-reference, contract-tests (incl. `msgpack_wire`, `wire_contract`), wav-reference |
| `cargo xtask verify rust` | `PASS` rust-format, rust-lint (workspace), rust-tests (workspace: 470 passed, 60 ignored) |
| `cargo xtask verify web` | `PASS` web-lint (cantos-studio, cantos), web-build (cantos-studio), web-build (cantos) |
| `cargo xtask preflight --changed-only --skip postgres` | `PASS` for every selected gate incl. mutation; `NOT_RUN postgres` (skipped) |
| `cargo xtask verify postgres` | `UNSUPPORTED postgres`: missing `initdb`, `pg_ctl`, `psql` |
| `cargo test -p cantos-api --release --test wire_bench -- --ignored --nocapture` | reporting harness; tables below |

The PostgreSQL tests are `#[ignore]`d outside the disposable-cluster runner, so the workspace
test count includes them as ignored, not passed.

## Payload size

`cargo test -p cantos-api --release --test wire_bench -- --ignored --nocapture`. Script IR
documents are canonical exports generated from the `two-scenes` fixture with Vietnamese lines
containing curly quotes, `"`, `\`, `…`, an emoji and apostrophes; "1 KiB" is the smallest generated
export (about 1.2 KiB). gzip is level 6.

| Payload | JSON B | MessagePack B | ratio | JSON gzip B | MessagePack gzip B | gzip ratio |
|---|---:|---:|---:|---:|---:|---:|
| review metadata | 425 | 398 | 0.936 | 241 | 245 | 1.017 |
| history page, 20 revisions | 8701 | 8125 | 0.934 | 852 | 856 | 1.005 |
| Revision, IR 1 KiB | 1707 | 1500 | 0.879 | 766 | 754 | 0.984 |
| SaveRevision, IR 1 KiB | 1458 | 1265 | 0.868 | 663 | 654 | 0.986 |
| ValidateScript, IR 1 KiB | 1381 | 1195 | 0.865 | 601 | 591 | 0.983 |
| Revision, IR 8 KiB | 9760 | 8657 | 0.887 | 1434 | 1398 | 0.975 |
| SaveRevision, IR 8 KiB | 9511 | 8422 | 0.886 | 1339 | 1305 | 0.975 |
| ValidateScript, IR 8 KiB | 9434 | 8352 | 0.885 | 1282 | 1249 | 0.974 |
| Revision, IR 64 KiB | 74360 | 66071 | 0.889 | 4174 | 4052 | 0.971 |
| SaveRevision, IR 64 KiB | 74111 | 65836 | 0.888 | 4065 | 3953 | 0.972 |
| ValidateScript, IR 64 KiB | 74034 | 65766 | 0.888 | 4004 | 3893 | 0.972 |
| Revision, IR 512 KiB | 591836 | 525967 | 0.889 | 24500 | 23733 | 0.969 |
| SaveRevision, IR 512 KiB | 591587 | 525732 | 0.889 | 24377 | 23612 | 0.969 |
| ValidateScript, IR 512 KiB | 591510 | 525662 | 0.889 | 24301 | 23534 | 0.968 |

Uncompressed, MessagePack is 11–13 % smaller for script payloads and 6–7 % for metadata. With
gzip the gap closes to about 2–3 % for scripts, and metadata is marginally larger. The API serves
no compression today; production edge compression is undecided.

## Codec time

Microseconds per operation, median (min–max of 7 rounds).

### Native, release build

| Payload | JSON encode | MessagePack encode | JSON decode | MessagePack decode |
|---|---:|---:|---:|---:|
| review metadata | 0.66 (0.65–0.67) | 0.40 (0.38–0.48) | 0.47 (0.46–0.52) | 0.43 (0.43–0.44) |
| history page, 20 revisions | 10.95 (8.57–16.30) | 4.97 (4.89–5.21) | 14.49 (12.64–20.75) | 12.19 (11.94–12.67) |
| Revision, IR 1 KiB | 2.53 (2.03–2.63) | 0.52 (0.48–0.52) | 3.29 (3.16–6.35) | 0.84 (0.83–0.85) |
| SaveRevision, IR 1 KiB | 1.88 (1.81–2.33) | 0.35 (0.28–0.35) | 2.92 (2.86–3.67) | 0.41 (0.40–0.53) |
| Revision, IR 8 KiB | 10.99 (10.77–11.12) | 0.48 (0.48–0.49) | 16.31 (16.25–17.16) | 3.47 (3.40–4.22) |
| SaveRevision, IR 8 KiB | 10.70 (10.57–10.89) | 0.35 (0.35–0.40) | 16.12 (15.90–18.30) | 3.21 (3.18–3.32) |
| Revision, IR 64 KiB | 86.33 (82.27–92.73) | 2.43 (2.42–2.51) | 125.14 (123.25–126.09) | 27.15 (26.76–27.55) |
| SaveRevision, IR 64 KiB | 82.98 (82.79–83.50) | 2.29 (2.28–2.73) | 124.13 (122.72–125.60) | 27.02 (26.81–27.50) |
| Revision, IR 512 KiB | 639.85 (636.18–645.34) | 16.42 (16.12–19.55) | 1000.68 (984.66–1003.33) | 230.86 (229.67–243.42) |
| SaveRevision, IR 512 KiB | 635.91 (633.04–648.70) | 16.03 (16.01–16.46) | 1006.71 (973.38–1009.75) | 219.37 (216.18–241.09) |

ValidateScript rows match SaveRevision within the spread (for example 64 KiB: 82.35 / 2.15 /
124.43 / 26.88 µs).

### WASM in the browser

The same payloads, exported with `CANTOS_WIRE_PAYLOADS`, were embedded in a temporary
`wasm32-unknown-unknown` release cdylib that calls `cantos_contracts::wire` (no imports), loaded
by a page in the in-app Chrome 152 and timed with `performance.now()` over at least 40 ms per
round. The harness was not committed. Only codec work is timed: fetching and copying bytes into
WASM memory is common to both codecs.

| Payload | JSON encode | MessagePack encode | JSON decode | MessagePack decode |
|---|---:|---:|---:|---:|
| review metadata | 0.49 (0.49–0.60) | 0.33 (0.33–0.35) | 0.77 (0.77–0.78) | 0.53 (0.53–0.54) |
| history page, 20 revisions | 9.42 (9.34–10.11) | 5.36 (5.27–5.42) | 17.29 (17.16–17.38) | 12.04 (11.96–12.21) |
| Revision, IR 1 KiB | 2.52 (2.51–2.54) | 0.35 (0.35–0.35) | 5.25 (5.21–5.33) | 0.83 (0.83–0.84) |
| SaveRevision, IR 1 KiB | 2.30 (2.27–2.33) | 0.21 (0.21–0.21) | 4.83 (4.79–4.83) | 0.51 (0.51–0.53) |
| Revision, IR 8 KiB | 13.60 (13.53–15.94) | 0.43 (0.43–0.43) | 29.79 (29.59–30.13) | 5.38 (5.35–5.44) |
| SaveRevision, IR 8 KiB | 13.38 (13.28–15.75) | 0.29 (0.29–0.30) | 29.25 (29.20–29.49) | 5.04 (5.03–5.11) |
| Revision, IR 64 KiB | 102.34 (101.95–102.34) | 2.37 (2.36–2.39) | 229.69 (228.13–231.64) | 43.75 (43.65–44.53) |
| SaveRevision, IR 64 KiB | 101.95 (101.56–102.54) | 2.36 (2.35–2.37) | 228.13 (227.73–231.25) | 42.09 (41.21–42.48) |
| ValidateScript, IR 64 KiB | 102.15 (101.76–103.52) | 2.28 (2.27–2.28) | 225.39 (224.61–230.86) | 41.60 (41.60–42.29) |
| Revision, IR 512 KiB | 825.00 (804.69–1018.75) | 16.24 (16.21–16.28) | 1850.00 (1825.00–2093.75) | 341.41 (339.06–353.13) |
| SaveRevision, IR 512 KiB | 814.06 (807.81–840.63) | 16.80 (16.77–16.85) | 1821.88 (1815.63–1856.25) | 348.44 (346.88–352.34) |
| ValidateScript, IR 512 KiB | 851.56 (815.62–882.81) | 17.24 (16.82–17.68) | 1843.75 (1821.87–1868.75) | 338.28 (336.72–350.78) |

The same module under Node 24 (V8, no browser) agreed within about 20 % (for example 512 KiB
Revision: 995 / 16.4 / 1868 / 297 µs).

In WASM MessagePack is faster at every size: from about 1.5× (metadata) to 50× (512 KiB) on
encode and from 1.4× (metadata) to 9× (small scripts) on decode, about 5× for larger scripts. In
absolute terms, a 64 KiB save plus head reload saves about 0.3 ms of main-thread codec time and a
512 KiB one about 2.3 ms. The server still parses `script_json` as JSON inside the Script IR
reader whatever the envelope, so the server-side saving is only the envelope.

## WASM bundle size

`cargo xtask build cantos-studio --web --release`, `dist/*_bg.wasm` (no wasm-opt). The
MessagePack row is the same source with `REVISION_CODEC = MessagePack`, built temporarily and
reverted.

| Build | raw B | gzip -9 B | zstd -19 B |
|---|---:|---:|---:|
| `develop` `44886f1` | 3 494 787 | 1 057 867 | 745 923 |
| this branch, JSON switch (shipped) | 3 521 803 | 1 067 957 | 755 364 |
| this branch, MessagePack switch | 3 611 915 | 1 093 574 | 771 837 |

- JSON switch vs `develop`: +27 016 B raw (+0.77 %), +10 090 B gzip (+0.95 %). The MessagePack
  decoder is not linked: rmp-serde's I/O error string is absent from the binary, while it is
  present in the MessagePack build. The delta is the negotiated transport path itself; it was not
  attributed function by function (no size profiler was run). Two measured iterations got here:
  without inlining the codec dispatch, the JSON build still linked the decoder (+13.3 KB gzip);
  decoding JSON through `from_str` reuses the parser instantiation the other routes already link.
- MessagePack switch vs JSON switch: +90 112 B raw (+2.56 %), +25 617 B gzip (+2.40 %), a one-time
  cost per cold cache.

## Browser runs

The release Studio builds above were served by a temporary Python stub (not committed) that
answers session, head, pinned revision, validation and save with
`scripts/reference_msgpack.py`'s independent encoder/decoder and logs each request. The stub is a
fake backend, so these runs are `interaction-tested` against a double, not `integration-tested`.
Each run: sign in with a synthetic token, open the fixture script, change a scene title to
`Sau buổi diễn — “đêm cuối” 🎭`, validate, confirm review, save; the MessagePack run then saved
again against a stub that answers `409 stale_revision`.

| Step | MessagePack switch | JSON switch |
| --- | --- | --- |
| session | JSON (route not negotiated) | JSON |
| head | `Accept: application/msgpack, application/json;q=0.5` → 2 297 B `application/msgpack` (`0x87`, 7-key map); UI showed revision 1 | `Accept: application/json` → JSON; revision 1 |
| validation | `Content-Type: application/msgpack`, body `0x81 "script_json"` decoded by the oracle → MessagePack `{issues: []}`; UI "đã đạt" | JSON both ways |
| save | `0x83` map with `expected_revision`, `operation_id`, `script_json` (Vietnamese intact) → MessagePack revision 2; UI showed revision 2 and the new title | JSON both ways; revision 2 |
| conflicting save | MessagePack request → `409` JSON `stale_revision`; UI kept the text and showed the newer-head prompt; exactly one request, no JSON retry | not run |

## Decision from the evidence

MessagePack stays **off** in the Studio (`REVISION_CODEC = Json`). The measured gains are real
but small for this product today: about 3 % of bytes after gzip, and at most a few milliseconds
of codec time for the largest scripts, against +25.6 KB gzip of bundle when enabled and no
end-to-end or PostgreSQL-backed run on this machine. The server, the client switch, the oracle
and the tests are complete, so turning it on later is a one-line change once ADR 0013's revisit
conditions are met.

## Not verified

- PostgreSQL-backed parity, replay, authorization and refusal tests, and the live socket check in
  `test_postgres.py`: implemented and compiled, not run here (no `initdb`/`pg_ctl`/`psql`). They
  run in the CI `postgres` job.
- End-to-end latency against the real API and PostgreSQL, with or without compression: not run.
- The real Axum host in a browser: only the stub backend was used.
- Any proxy, CDN or edge compression behaviour with `application/msgpack` bodies.
- Other browsers and engines, mobile devices, sustained or concurrent load.
- Fuzzing of the decoder (`fuzz-tested` is not claimed); the hostile-input tests are fixed cases.
