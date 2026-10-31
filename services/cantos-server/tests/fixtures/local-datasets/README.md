# Local dataset inputs

Versioned synthetic inputs for the explicit [localhost dataset lane](../../../../../docs/deployment/localhost-datasets.md).
`registry.json` owns names/cardinality bounds, `substring.json` owns original text and handwritten
membership, and `checks.sql` is native read-only inspection. Normal data is admitted by current
Catalog handlers. Negative requests are emitted separately by the fixture, never inserted as
invalid normal rows. No credentials, downloaded text, audio or runtime database belongs here.

The publication-access profile also uses production metadata publication and viewer-policy
commands over explicit synthetic rights/session fixtures. Its opt-in measurement campaign
and temporary index experiments are described in the linked runbook; no production budget,
index or cache adoption follows from those measurements.
