//! Side-effect-free, controlled Studio composition; primitives come from the admitted UI system.
use super::{
    copy::{self, Key},
    Draft, Intent, Projection, Stage,
};
use crate::ui::{ActionButton, BlockReason, DiscardDraftDialog, DiscardTarget};
use cantos_ui::{Badge, ButtonVariant, FieldState, TextField};
use leptos::{ev::Event, prelude::*};

#[component]
pub fn ArtworkForm(
    model: Signal<Projection>,
    draft: RwSignal<Draft>,
    locale: Signal<crate::localization::Locale>,
    owner: Signal<bool>,
    blocked: Signal<bool>,
    on_intent: Callback<Intent>,
    on_file: Callback<Event>,
    clear_open: RwSignal<bool>,
    #[prop(into, default = false.into())] can_reconcile: Signal<bool>,
    #[prop(into, default = false.into())] can_replay: Signal<bool>,
) -> impl IntoView {
    let label = move |key| copy::text(key, locale.get()).to_owned();
    let locked = Signal::derive(move || blocked.get() || !owner.get());
    let fields = Signal::derive(move || {
        if locked.get() {
            FieldState::ReadOnly
        } else {
            FieldState::Editable
        }
    });
    let declarations = Signal::derive(move || {
        if locked.get() || model.with(|m| m.intent.is_some()) {
            FieldState::ReadOnly
        } else {
            FieldState::Editable
        }
    });
    let problem = move |p| {
        model
            .with(|m| m.problem.filter(|v| *v == p))
            .map(|p| copy::problem(p, locale.get()).to_owned())
    };
    let discard_target = Memo::new(move |_| {
        DiscardTarget::Artwork(model.with(|m| {
            m.current
                .as_ref()
                .map(|r| format!("{} · r{}", r.work_id, r.work_revision))
                .unwrap_or_default()
        }))
    });
    view! { <section id="studio-artwork" class="live-section studio-artwork" aria-labelledby="artwork-title">
        <header class="artwork-heading"><div><h3 id="artwork-title">{move ||label(Key::Title)}</h3><p>{move ||label(Key::Private)}</p></div></header>
        <div id="artwork-status" tabindex="-1" role="status" aria-live="polite" aria-atomic="true"><Badge>{move ||copy::stage(model.with(|m|m.stage),locale.get())}</Badge></div>
        <Show when=move ||!owner.get()><p>{move ||label(Key::Owner)}</p></Show>
        <div class="artwork-columns">
            <figure class="artwork-current"><figcaption>{move ||label(Key::Current)}</figcaption>
                {move ||model.with(|m| match &m.current {
                    Some(current) => match &current.artwork {
                        Some(art) => view! { <div>{match &m.current_url {
                            Some(url) => view!{<img class="artwork-image" src=url.clone() alt=art.alt_text.clone()/>}.into_any(),
                            None => view!{<p>{label(Key::UnavailablePreview)}</p>}.into_any(),
                        }}<p>{art.alt_text.clone()}</p>{m.current_media.as_ref().map(|media|view!{<p>{copy::media_facts(media.byte_length,media.width_px,media.height_px,super::mime_name(media.content_type),locale.get())}</p>})}<small>{format!("r{} · {}",current.work_revision,art.image_asset_id)}</small></div>}.into_any(),
                        None => view!{<p class="artwork-placeholder">{label(Key::Empty)}</p><small>{format!("r{}",current.work_revision)}</small>}.into_any(),
                    },
                    None => view!{<p class="artwork-placeholder">{copy::stage(m.stage,locale.get())}</p>}.into_any(),
                })}
            </figure>
            <div class="artwork-replacement"><h4>{move ||label(Key::Replacement)}</h4>
                <label class="cui-field-label" for="artwork-file">{move ||label(Key::File)}</label>
                <input id="artwork-file" type="file" accept="image/png,image/jpeg" disabled=move ||locked.get()
                    aria-describedby="artwork-bounds artwork-file-error" aria-invalid=move ||model.with(|m|matches!(m.problem,Some(super::Problem::File|super::Problem::Dimensions|super::Problem::Decode)))
                    on:change=move |e|on_file.run(e)/>
                <p id="artwork-bounds" class="cui-field-help">{move ||label(Key::Bounds)}</p>
                <p id="artwork-file-error" class="cui-field-error" role="alert" hidden=move ||!model.with(|m|matches!(m.problem,Some(super::Problem::File|super::Problem::Dimensions|super::Problem::Decode)))>{move ||model.with(|m|m.problem).filter(|p|matches!(p,super::Problem::File|super::Problem::Dimensions|super::Problem::Decode)).map(|p|copy::problem(p,locale.get()))}</p>
                {move ||model.with(|m|m.local.clone()).map(|file|view!{
                    <img class="artwork-image" src=file.url alt=""/>
                    <p class="artwork-file-facts">{copy::file_facts(&file.name,file.bytes.len() as u64,file.width,file.height,super::mime_name(file.mime),locale.get())}</p>
                })}
                <Show when=move ||model.with(|m|m.transferred.is_some())><p>{move ||model.with(|m|m.transferred.zip(m.local.as_ref()).map(|(bytes,f)|copy::transferred_bytes(bytes,f.bytes.len() as u64,locale.get())))}</p></Show>
                <ActionButton id="artwork-discard" locale=locale reason=BlockReason::Artwork(Key::NeedLocal) blocked=Signal::derive(move ||locked.get()||model.with(|m|m.local.is_none())) variant=ButtonVariant::Text
                    on_press=Callback::new(move |_|on_intent.run(Intent::DiscardLocal))>{move ||label(Key::Discard)}</ActionButton>
            </div>
        </div>
        <div class="artwork-fields">
            <TextField id="artwork-alt" label=Signal::derive(move ||label(Key::Alt)) value=Signal::derive(move ||draft.with(|d|d.alt.clone()))
                state=fields required=true help=Signal::derive(move ||Some(label(Key::AltHelp))) error=Signal::derive(move ||problem(super::Problem::Alt))
                on_input=Callback::new(move |v|draft.update(|d|d.alt=v))/>
            <TextField id="artwork-provenance" label=Signal::derive(move ||label(Key::Provenance)) value=Signal::derive(move ||draft.with(|d|d.provenance.clone()))
                state=declarations required=true help=Signal::derive(move ||Some(label(Key::ProvenanceHelp))) error=Signal::derive(move ||problem(super::Problem::Provenance))
                on_input=Callback::new(move |v|draft.update(|d|d.provenance=v))/>
            <TextField id="artwork-rights" label=Signal::derive(move ||label(Key::Rights)) value=Signal::derive(move ||draft.with(|d|d.rights.clone()))
                state=declarations required=true help=Signal::derive(move ||Some(label(Key::RightsHelp))) error=Signal::derive(move ||problem(super::Problem::Rights))
                on_input=Callback::new(move |v|draft.update(|d|d.rights=v))/>
        </div>
        <Show when=move ||model.with(|m|m.intent.is_some())><p>{move ||label(Key::PreparedHelp)}</p><details><summary>{move ||label(Key::IntentReceipt)}</summary><small>{move ||model.with(|m|m.intent.as_ref().map(|i|i.operation_id.clone()))}</small></details></Show>
        <Show when=move ||model.with(|m|m.compared_revision.is_some())><p>{move ||label(Key::Compare)}{move ||model.with(|m|format!(" · r{} / r{}",m.compared_revision.unwrap_or_default(),m.reported_revision.or_else(||m.current.as_ref().map(|r|r.work_revision)).unwrap_or_default()))}</p></Show>
        <div class="toolbar artwork-actions">
            <ActionButton id="artwork-prepare" locale=locale reason=BlockReason::Artwork(Key::NeedFile) blocked=Signal::derive(move ||locked.get()||model.with(|m|!m.can_prepare())) on_press=Callback::new(move |_|on_intent.run(Intent::Prepare))>{move ||label(Key::Prepare)}</ActionButton>
            <ActionButton id="artwork-upload" locale=locale reason=BlockReason::Artwork(Key::NeedIntent) blocked=Signal::derive(move ||locked.get()||model.with(|m|!m.can_upload())) on_press=Callback::new(move |_|on_intent.run(Intent::Upload))>{move ||label(Key::Upload)}</ActionButton>
            <ActionButton id="artwork-select" locale=locale reason=BlockReason::Artwork(Key::NeedAsset) blocked=Signal::derive(move ||locked.get()||model.with(|m|m.asset.is_none()||m.current.is_none())) on_press=Callback::new(move |_|on_intent.run(Intent::Select))>{move ||label(Key::Select)}</ActionButton>
            <ActionButton id="artwork-refresh" locale=locale reason=BlockReason::Artwork(Key::NeedOwner) blocked=locked variant=ButtonVariant::Tonal on_press=Callback::new(move |_|on_intent.run(Intent::Refresh))>{move ||label(Key::Refresh)}</ActionButton>
            <ActionButton id="artwork-clear" locale=locale reason=BlockReason::Artwork(Key::NeedCurrent) focus_on_press=true blocked=Signal::derive(move ||locked.get()||model.with(|m|m.current.as_ref().is_none_or(|r|r.artwork.is_none()))) variant=ButtonVariant::Text on_press=Callback::new(move |_|on_intent.run(Intent::Clear))>{move ||label(Key::Clear)}</ActionButton>
        </div>
        <Show when=move ||model.with(|m|m.stage==Stage::Unknown)><p>{move ||label(Key::Operation)}{move ||model.with(|m|m.operation.clone())}</p><p>{move ||label(Key::PendingHelp)}</p>
            <div class="toolbar"><ActionButton id="artwork-reconcile" locale=locale reason=BlockReason::Artwork(Key::NeedRecovery) blocked=Signal::derive(move ||!owner.get()||!can_reconcile.get()||model.with(|m|m.stage!=Stage::Unknown)) variant=ButtonVariant::Tonal on_press=Callback::new(move |_|on_intent.run(Intent::Reconcile))>{move ||label(Key::Reconcile)}</ActionButton>
            <ActionButton id="artwork-replay" locale=locale reason=BlockReason::Artwork(Key::NeedRecovery) blocked=Signal::derive(move ||!owner.get()||!can_replay.get()||model.with(|m|m.stage!=Stage::Unknown)) variant=ButtonVariant::Tonal on_press=Callback::new(move |_|on_intent.run(Intent::Replay))>{move ||label(Key::Replay)}</ActionButton></div>
        </Show>
        <DiscardDraftDialog open=clear_open locale=locale blocked=locked
            target=Signal::derive(move ||discard_target.get())
            on_confirm=Callback::new(move |()|{on_intent.run(Intent::ConfirmClear);true})
            on_error=Callback::new(move |_|on_intent.run(Intent::CancelClear))/>

    </section> }
}
