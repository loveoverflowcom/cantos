//! Debug-only injected composition preview in the existing Studio runtime. No HTTP, auth or storage.
use super::*;
use cantos_ui::Theme;
use leptos::prelude::*;
pub const SCENARIOS: &[(Stage, &str)] = &[
    (Stage::Empty, "empty"),
    (Stage::Current, "current"),
    (Stage::Replacement, "replacement"),
    (Stage::Validating, "validating"),
    (Stage::Preparing, "preparing"),
    (Stage::Prepared, "prepared"),
    (Stage::Uploading, "uploading"),
    (Stage::AwaitingAcknowledgement, "awaiting-acknowledgement"),
    (Stage::Admitted, "admitted"),
    (Stage::Failed, "failed"),
    (Stage::Expired, "expired"),
    (Stage::Unknown, "unknown"),
    (Stage::Conflict, "conflict"),
];
const IMAGE: &str="data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 320 240'%3E%3Crect width='320' height='240' fill='%238472a0'/%3E%3Ctext x='25' y='130' fill='white' font-size='24'%3ECantos fixture%3C/text%3E%3C/svg%3E";
fn fixture(stage: Stage) -> Projection {
    let has_current = !matches!(stage, Stage::Empty);
    let local = !matches!(stage, Stage::Empty | Stage::Current);
    let declarations = ImageDeclarations {
        provenance: "Synthetic fixture".into(),
        rights_refs: vec!["rights_fixture".into()],
    };
    let request = CreateImageIntentRequest {
        operation_id: "00000000-0000-4000-8000-000000000003".into(),
        id: "00000000-0000-4000-8000-000000000002".into(),
        expected_sha256: "ab".repeat(32),
        expected_byte_length: 128,
        content_type: ImageMime::Png,
        declarations: declarations.clone(),
    };
    let intent = ImageIntentDocument {
        schema_version: "1.0.0".into(),
        id: request.id.clone(),
        work_id: "00000000-0000-4000-8000-000000000001".into(),
        owner_id: "fixture-owner".into(),
        operation_id: request.operation_id.clone(),
        request,
        created_at_ms: 0,
        expires_at_ms: 1_800_000,
        digest: "ii-r1:sha256:fixture".into(),
        profile_id: "cover-image-v1".into(),
    };
    let asset = ImageAssetDocument {
        schema_version: "1.0.0".into(),
        id: "00000000-0000-4000-8000-000000000004".into(),
        intent_id: intent.id.clone(),
        work_id: intent.work_id.clone(),
        owner_id: intent.owner_id.clone(),
        verified_media: ImageVariantMetadata {
            sha256: "ab".repeat(32),
            byte_length: 128,
            content_type: ImageMime::Png,
            width_px: 320,
            height_px: 240,
            profile_id: "cover-image-v1".into(),
        },
        declarations,
        admitted_at_ms: 10,
        digest: "ia-r1:sha256:fixture".into(),
    };
    Projection {
        intent_state: match stage {
            Stage::Expired => Some(ImageIntentState::Expired),
            Stage::Admitted | Stage::Conflict => Some(ImageIntentState::Admitted),
            Stage::Prepared | Stage::Uploading | Stage::AwaitingAcknowledgement | Stage::Unknown => {
                Some(ImageIntentState::Pending)
            }
            _ => None,
        },
        intent: matches!(
            stage,
            Stage::Prepared
                | Stage::Uploading
                | Stage::AwaitingAcknowledgement
                | Stage::Admitted
                | Stage::Expired
                | Stage::Unknown
                | Stage::Conflict
        )
        .then_some(intent),
        asset: matches!(stage, Stage::Admitted | Stage::Conflict).then_some(asset),
        operation: (stage == Stage::Unknown).then(|| "00000000-0000-4000-8000-000000000003".into()),
        reported_revision: (stage == Stage::Conflict).then_some(7),
        stage,
        current: Some(WorkArtworkResponse {
            work_id: "00000000-0000-4000-8000-000000000001".into(),
            work_revision: 7,
            artwork: has_current.then(|| WorkArtworkSelection {
                image_asset_id: "00000000-0000-4000-8000-000000000004".into(),
                image_asset_digest: "fixture-only".into(),
                alt_text: "Ánh đèn cuối sân khấu, bản minh họa hiện tại".into(),
            }),
            changed_at_ms: None,
        }),
        current_url: has_current.then(|| IMAGE.into()),
        local: local.then(|| LocalImage {
            name: "anh_den_cuoi_san_khau.png".into(),
            bytes: Arc::new(vec![0; 128]),
            sha256: "ab".repeat(32),
            mime: ImageMime::Png,
            width: 320,
            height: 240,
            url: IMAGE.into(),
        }),
        transferred: (stage == Stage::Uploading).then_some(64),
        compared_revision: (stage == Stage::Conflict).then_some(5),
        ..Projection::default()
    }
}
pub fn requested() -> bool {
    web_sys::window()
        .and_then(|w| w.location().search().ok())
        .is_some_and(|q| q == "?preview=studio-artwork")
}
#[component]
pub fn Preview(theme: RwSignal<Theme>) -> impl IntoView {
    let model = RwSignal::new(fixture(Stage::Empty));
    let draft = RwSignal::new(Draft {
        alt: "Núi non trong sương dưới ánh trăng, minh họa Ánh đèn cuối sân khấu".into(),
        provenance: "Bản minh họa gốc do họa sĩ bàn giao; fixture không phải chứng nhận quyền."
            .into(),
        rights: "rights_fixture".into(),
    });
    let english = RwSignal::new(false);
    let clear_open = RwSignal::new(false);
    view! {
    <main class="studio-workspace"><h1>"studio-artwork · injected preview (debug only)"</h1><p>"Recipe v1 · synthetic fixture · no API/storage. Selecting a scenario explicitly resets its projection; theme and locale retain text."</p>
        <label for="preview-stage">"Scenario"</label><select id="preview-stage" on:change=move |e|{let value=event_target_value(&e);if let Some((stage,_))=SCENARIOS.iter().find(|(_,s)|*s==value){model.set(fixture(*stage));}}>{SCENARIOS.iter().map(|(_,id)|view!{<option value=*id>{*id}</option>}).collect_view()}</select>
        <label for="preview-theme">"Theme"</label><select id="preview-theme" on:change=move |e|if let Some(t)=Theme::from_attribute(&event_target_value(&e)){theme.set(t);}>
            <option value="light">"Light"</option><option value="dark">"Dark"</option></select>
        <label for="preview-locale">"Locale"</label><select id="preview-locale" on:change=move |e|english.set(event_target_value(&e)=="en")><option value="vi-VN">"vi-VN"</option><option value="en">"en"</option></select>
        <form::ArtworkForm model=Signal::derive(move ||model.get()) draft=draft english=english owner=Signal::derive(||true)
            blocked=Signal::derive(move ||matches!(model.with(|m|m.stage),Stage::Validating|Stage::Preparing|Stage::Uploading|Stage::AwaitingAcknowledgement|Stage::Unknown))
            clear_open=clear_open can_reconcile=Signal::derive(move ||model.with(|m|m.stage==Stage::Unknown)) can_replay=Signal::derive(move ||model.with(|m|m.stage==Stage::Unknown)) on_file=Callback::new(|_|{}) on_intent=Callback::new(move |intent|match intent {
                Intent::Clear=>clear_open.set(true),Intent::ConfirmClear|Intent::CancelClear=>clear_open.set(false),_=>{},
            })/>
    </main>}
}
