# Deferred expansion

These ideas are outside the first production-to-publication and listening workflow. Promote an item only when real usage, implementation evidence, or an explicit product decision makes it necessary. Give it a separate review boundary rather than expanding an active MVP item.

| Idea | Why deferred | Evidence needed to promote |
| --- | --- | --- |
| Independent API microservices, event buses, and generic distributed orchestration | The modular monolith serves the current catalog/admission/publication flow; it has no synthesis jobs. #203's external Forge adapter is a concrete producer boundary, not permission for a broader service split. | Measured ownership, deployment, scaling, or reliability limits that a smaller change cannot address |
| Provider auto-routing, generation batching, and local model tuning | Advanced external Forge/producer work; #203 first needs one bounded provider path with recovery/cost safety. Never backend TTS. | Quality, latency, cost, or hardware data from actual production samples |
| Real-time collaborative Studio and full DAW controls | Single-creator editing and focused production controls deliver the first workflow. | Demonstrated collaborative or editing tasks blocked by the MVP |
| Studio Mobile | Studio is web-only; no client, scaffold or build/test target is maintained. | Explicit future roadmap request; usage evidence alone does not authorize implementation |
| Direct Narrative Forge integration and additional import formats | Versioned Script IR keeps integration possible without coupling projects now. | A concrete source/consumer, fixtures, and compatibility ownership |
| Personalized recommendations, subscriptions, payments, and social features | Discovery and reliable listening must work first. | Defined business rules and audience needs after usable published content exists |
| Automated perceptual QC, music generation, and advanced mastering | Measurable QC plus creator listening/approval provides the initial gate. | Evaluation material and a verified quality improvement over the current review process |
| Binaural, 3D or object-based spatial mixes | Conservative, mono-compatible stereo placement ([#53](https://github.com/loveoverflowcom/cantos/issues/53)) comes first; phone speakers fold to mono. | Listening evidence on target devices that spatial rendering improves comprehension or immersion without harming mono playback |
| Podcast/RSS syndication of published releases | Theatre is the distribution surface; external feeds add rights, metadata and retraction obligations. | A product decision on external distribution, rights scope covering it, and a retraction policy for syndicated copies |
| Dedicated recording UI and recording-technique curriculum | Existing intake already accepts authorized externally recorded media; no new recording product is needed for that. | A concrete authoring/recording consumer beyond current file admission, with rights and external conditioning requirements |
| Multi-region delivery, CDN tuning, archival automation, and aggressive deduplication | Correct delivery, version retention, and safe uploads precede optimization. | Storage/traffic/cost measurements and a retention policy that protects active playback and offline clients |
| Wearables, car interfaces, live broadcasting, and DRM | They have no immediate MVP consumer. | An explicit platform/distribution requirement and native runtime validation capacity |
| DBSP for a concrete metadata read model | The [old TTS job-summary adapter](../../decisions/0017-experimental-dbsp-job-summary.md) is historical/retired, not a production-adoption candidate. The generic library is optional. | A current non-synthesis read workload that misses a measured PostgreSQL budget, plus reviewed consistency, lag, rebuild/fallback and retention costs |

Data loss, permission bypass, unbounded billable retries, invalid publication, unusable media, and corrupt offline state are correctness fixes—not deferred optimization.
