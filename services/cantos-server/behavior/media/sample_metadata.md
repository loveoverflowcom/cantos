# Retired: sample metadata disclosure

The [owner-approved sample slice](../../../../docs/decisions/0051-metadata-only-public-discovery.md#decision) requires explicit independent Work and unit decisions, live visibility and active owner, and no playback without an eligible release. This model abstracts the six approved identities to `scope`, the accepted release policy to `released`, and source/rights/snapshot checks to the precondition for a disclosure. It also models a reserved principal, explicit input confirmations, refusal to attach an identity after reservation, and publication followed by permanent withdrawal. Confirmation flags represent supplied assertions, not proof that they are true. It does not model database transactions, sessions, evidence bytes, operator identity, audit integrity, idempotent replay, client behavior, real audio or deployment. Release eligibility is an input fact, not derived or granted here. The model is a draft expectation; a passing simulation is not implementation conformance.


ADR 0054 D07 retires this sample-only public lane in S05 (#651). Anonymous Discovery now reads
only the generic generation facade; sample attestations and private seed provenance cannot
admit a Work or Unit. The stable ID and model remain as historical, draft-origin evidence;
there is no production observation or compatibility route for it. No history is converted into
publication authority. The successor is `cantos.publishing.metadata-generation` (draft,
`model: null`); its core and real-file/HTTP tests do not establish model conformance.
