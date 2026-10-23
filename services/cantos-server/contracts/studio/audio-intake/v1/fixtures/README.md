# Studio audio intake v1 transport fixtures

These hand-authored JSON documents contain synthetic identifiers, timestamps, digests and
media metadata only. There are no source manuscripts, real accounts, media files, generated
audio or credentials. The `sha256` value does not attest to any actual recording.

- `intent-request.json`: explicit owner-selected profile plus current revision/content pins.
- `intent-response.json`: immutable scope and authenticated same-origin upload/status paths.
- `media-metadata.json`: synthetic observed-facts wire shape for the selected Opus profile.
- `receipt-response.json`: immutable private acceptance receipt shape, without a storage URL.
- `status-empty.json`: intent with no accepted receipt, represented by explicit `null`.
- `status-accepted.json`: the same intent and receipt facts together.
- `rejected-aac-media.json`: negative profile/container/codec fixture; it must never satisfy
  the current intake schema.

Fixtures check contract compatibility, not file acceptance, decoding, listening quality,
rights, object durability or publication. See the
[narrow development contract](../README.md).
