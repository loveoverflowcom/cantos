# Deferred expansion

These ideas are outside the first production-to-publication and listening workflow. Promote an item only when real usage, implementation evidence, or an explicit product decision makes it necessary. Give it a separate review boundary rather than expanding an active MVP item.

| Idea | Why deferred | Evidence needed to promote |
| --- | --- | --- |
| Independent microservices, event buses, and distributed orchestration | A modular backend with durable PostgreSQL jobs serves the current product flow. | Measured ownership, deployment, scaling, or reliability limits that a smaller change cannot address |
| Provider auto-routing, large catalogs, generation batching, and local model tuning | Start with usable provider paths and correct recovery/cost behavior. | Quality, latency, cost, or hardware data from actual production samples |
| Real-time collaborative Studio, full DAW controls, and mobile Studio | Single-creator editing and focused production controls deliver the first workflow. | Demonstrated collaborative or editing tasks blocked by the MVP |
| Direct Narrative Forge integration and additional import formats | Versioned Script IR keeps integration possible without coupling projects now. | A concrete source/consumer, fixtures, and compatibility ownership |
| Personalized recommendations, subscriptions, payments, and social features | Discovery and reliable listening must work first. | Defined business rules and audience needs after usable published content exists |
| Automated perceptual QC, music generation, and advanced mastering | Measurable QC plus creator listening/approval provides the initial gate. | Evaluation material and a verified quality improvement over the current review process |
| Binaural, 3D or object-based spatial mixes | Conservative, mono-compatible stereo placement ([#53](https://github.com/loveoverflowcom/cantos/issues/53)) comes first; phone speakers fold to mono. | Listening evidence on target devices that spatial rendering improves comprehension or immersion without harming mono playback |
| Podcast/RSS syndication of published releases | Theatre is the distribution surface; external feeds add rights, metadata and retraction obligations. | A product decision on external distribution, rights scope covering it, and a retraction policy for syndicated copies |
| Recording guidance and import for creator-recorded voices | The pipeline synthesizes speech; human recording technique does not apply yet. | A decision to accept recorded performances, with intake, rights and conditioning requirements |
| Multi-region delivery, CDN tuning, archival automation, and aggressive deduplication | Correct delivery, version retention, and safe uploads precede optimization. | Storage/traffic/cost measurements and a retention policy that protects active playback and offline clients |
| Wearables, car interfaces, live broadcasting, and DRM | They have no immediate MVP consumer. | An explicit platform/distribution requirement and native runtime validation capacity |
| Production adoption of the DBSP job-summary read model, or DBSP for other reads | The [experimental slice](../../decisions/0017-experimental-dbsp-job-summary.md) is local/opt-in; the outbox is never pruned, every process rebuilds its own in-memory view, and no workload needs it yet. | A measured read workload that misses a stated budget on the improved PostgreSQL path; outbox retention with a consumer registry; monitored lag/rebuild/fallback budgets; a decision on `serde_json/arbitrary_precision` in shipped binaries; checkpoint or per-process state operations; logical-replication CDC if trigger write cost matters |

Data loss, permission bypass, unbounded billable retries, invalid publication, unusable media, and corrupt offline state are correctness fixes—not deferred optimization.
