# Content Identity v1 fixtures

Synthetic Vietnamese content only. `cases.json` names literal schema acceptance/rejection
cases. Text digest literals were computed independently with Python hashlib over compact,
sorted, UTF-8 JSON and the prefixes in [the contract](../../../content-v1.md).

`studio_contract` compares schema, DTO admission and independent digest calculations. The
recording-extra-field fixture guards Serde's empty-variant construction path. Latest Script IR
sequence zero is structurally readable as a raw DTO but rejected by domain admission. Schema
character bounds do not replace domain UTF-8 byte checks or backend owner/evidence checks.
