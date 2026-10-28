    /// Run only with the authorized external #427 file, never a checked-in manuscript or media.
    #[test]
    #[ignore = "requires the externally supplied sample-catalog.json"]
    fn supplied_source_has_exact_approved_membership() {
        use super::super::{
            ContentUnit, ContentUnitId, NewContentUnit, NewWork, OwnerId, UnitDetails, Work,
            WorkDetails, WorkId, WorkKind,
        };
        let path = std::env::var_os("CANTOS_APPROVED_SAMPLE_INPUT").expect("sample input path");
        let mut bytes = std::fs::read(path).expect("read approved sample");
        let source = ApprovedSampleSource::parse(&bytes).expect("pinned sample");
        let projected = source.sample_seed_snapshots(1).expect("valid sample snapshots");
        assert_eq!(projected.len(), 6);
        assert_eq!(projected.iter().map(|(_, units)| units.len()).sum::<usize>(), 12);
        for (work, units) in &projected {
            assert_eq!(work.owner_id().as_str(), "cantos_sample_seed_v1");
            assert!(source.matches_work(work));
            assert!(units.iter().all(|unit| source.matches_unit(unit)));
            assert!(units.iter().all(|unit| unit.details().source.is_none()
                && unit.details().script.is_none() && unit.details().content_digest.is_none()));
        }
        assert_eq!(source.works.len(), 6);
        assert_eq!(
            source
                .works
                .iter()
                .map(|work| work.chapters.len())
                .sum::<usize>(),
            12
        );
        for item in &source.works {
            let details = WorkDetails {
                title: item.title.clone(),
                synopsis: item.description.clone(),
                language: item.language.clone(),
                visibility: Visibility::Public,
                rights_refs: vec![],
            };
            let work = Work::new(NewWork {
                id: WorkId::parse(&sample_identity(&item.sample_key, "work")).unwrap(),
                kind: WorkKind::parse(&item.kind).unwrap(),
                owner_id: OwnerId::parse("sample_owner").unwrap(),
                details: details.clone(),
                at: 1,
            })
            .unwrap();
            assert!(source.matches_work(&work));
            let admitted = || PublishFacts {
                actor: work.owner_id(),
                owner_active: true,
                work: &work,
                unit: None,
                source: &source,
                evidence: MetadataEvidence::Confirmed,
            };
            assert!(authorize_publish(admitted()).is_ok());
            assert!(matches!(
                authorize_publish(PublishFacts {
                    owner_active: false,
                    ..admitted()
                }),
                Err(PublishDenial::InactiveOwner)
            ));
            let other = OwnerId::parse("other_owner").unwrap();
            assert!(matches!(
                authorize_publish(PublishFacts {
                    actor: &other,
                    ..admitted()
                }),
                Err(PublishDenial::NotOwner)
            ));
            assert!(matches!(
                authorize_publish(PublishFacts {
                    evidence: MetadataEvidence::MissingOrRevoked,
                    ..admitted()
                }),
                Err(PublishDenial::EvidenceMissingOrRevoked)
            ));
            let mut changed = details;
            changed.title.push('!');
            let changed_work = work.edit(1, changed, 2).unwrap();
            assert!(!source.matches_work(&changed_work));
            assert!(matches!(
                authorize_publish(PublishFacts {
                    work: &changed_work,
                    ..admitted()
                }),
                Err(PublishDenial::OutsideApprovedSample)
            ));
            for chapter in &item.chapters {
                let details = UnitDetails {
                    title: chapter.title.clone(),
                    description: chapter.description.clone(),
                    source: None,
                    script: None,
                    content_digest: None,
                };
                let unit = ContentUnit::new(
                    &work,
                    NewContentUnit {
                        id: ContentUnitId::parse(&sample_identity(&chapter.sample_key, "chapter"))
                            .unwrap(),
                        kind: work.kind().content_unit_kind(),
                        details,
                        position: chapter.position,
                        at: 1,
                    },
                )
                .unwrap();
                assert!(source.matches_unit(&unit));
                assert!(authorize_publish(PublishFacts {
                    unit: Some(&unit),
                    ..admitted()
                })
                .is_ok());
                let mut edited = unit.details().clone();
                edited.title.push('!');
                let altered = unit.edit(&work, 1, edited, 2).unwrap();
                assert!(matches!(
                    authorize_publish(PublishFacts {
                        unit: Some(&altered),
                        ..admitted()
                    }),
                    Err(PublishDenial::OutsideApprovedSample)
                ));
            }
        }
        bytes[0] ^= 1;
        assert!(matches!(
            ApprovedSampleSource::parse(&bytes),
            Err(SampleSourceError::DigestMismatch)
        ));
    }
