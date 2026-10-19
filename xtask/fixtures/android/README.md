# Android parser fixtures

`badging-demo.txt`, `badging-demo-qa.txt` and `apksigner-demo.txt` retain historical output.
The development/release files derive synthetic package/version/component projections from
those records for #288 policy tests; they are not observations of new installed artifacts.
Signer fingerprints are synthetic. Actual build and signing evidence is recorded separately.
