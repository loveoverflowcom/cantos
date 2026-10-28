# Local dataset inputs

Versioned synthetic inputs for the explicit [localhost dataset lane](../../../../../docs/deployment/localhost-datasets.md).
`registry.json` owns names/cardinality bounds, `substring.json` owns original text and handwritten
membership, and `checks.sql` is native read-only inspection. Normal data is admitted by current
Catalog handlers. Negative requests are emitted separately by the fixture, never inserted as
invalid normal rows. No credentials, downloaded text, audio or runtime database belongs here.
