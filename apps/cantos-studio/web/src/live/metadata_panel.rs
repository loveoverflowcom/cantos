//! App-owned in-place review over the shared controls; all mutations use the workspace request owner.
use super::{
    metadata_publication::MetadataReview,
    workspace::{uuid, Dispatch},
    Command, Facts, Phase,
};
use crate::{
    contract::{MetadataPublicationLifecycle as Lifecycle, MetadataPublicationRequest as Request},
    localization::generated::live::{text, Key},
    ui::{ActionButton, BlockReason, ReviewCheckbox},
};
use cantos_ui::{ButtonVariant, IconName};
use leptos::prelude::*;
use wasm_bindgen::JsCast;

fn focus(id: &str) {
    if let Some(node) = web_sys::window()
        .and_then(|w| w.document())
        .and_then(|d| d.get_element_by_id(id))
        .and_then(|e| e.dyn_into::<web_sys::HtmlElement>().ok())
    {
        let _ = node.focus();
    }
}

#[component]
pub(super) fn MetadataPanel(
    facts: RwSignal<Facts>,
    dispatch: Dispatch,
    locale: Signal<crate::localization::Locale>,
    blocked: Signal<bool>,
    unsaved: Signal<bool>,
) -> impl IntoView {
    let review = RwSignal::new(None::<MetadataReview>);
    let confirmed = RwSignal::new(false);
    let selection_error = RwSignal::new(false);
    let display_only = RwSignal::new(false);
    let owner = Signal::derive(move || {
        facts.with(|f| {
            f.work
                .as_ref()
                .is_some_and(|w| w.owner_id == dispatch.state.with(|s| s.actor().to_owned()))
        })
    });
    // The mounted workspace survives preference/task changes. Only an actual Work/account change resets selection.
    Effect::new(move |_| {
        let work = facts.with(|f| f.work.as_ref().map(|w| w.id.clone()));
        if review.with_untracked(|r| {
            r.as_ref()
                .is_some_and(|r| Some(r.work()) != work.as_deref())
        }) || !owner.get()
        {
            review.set(None);
            confirmed.set(false);
            display_only.set(false);
        }
    });
    Effect::new(
        move |previous: Option<Option<(String, u64, u64, String)>>| {
            let subject = facts.with(|f| {
                let unit = f.unit.as_ref()?;
                let state = f.metadata.current.as_ref()?;
                let entry = state
                    .generation
                    .as_ref()?
                    .entries
                    .iter()
                    .find(|e| e.unit_id == unit.id)?;
                let release = f.publication.active.as_ref()?.release.as_ref()?;
                Some((
                    unit.id.clone(),
                    state.version,
                    entry.revision,
                    release.id.clone(),
                ))
            });
            if previous.as_ref() != Some(&subject) {
                display_only.set(false);
            }
            subject
        },
    );
    let current = Signal::derive(move || facts.with(|f| f.metadata.current.clone()));
    let archived = Signal::derive(move || {
        facts.with(|f| f.work.as_ref().is_some_and(|w| w.lifecycle == "archived"))
    });
    // A write receipt never means the before-state was empty. Keep the last read comparison
    // while current state is unread; a fresh GET replaces it without changing the frozen intent.
    let comparison = Signal::derive(move || {
        current
            .get()
            .or_else(|| review.with(|r| r.as_ref().map(|r| r.baseline().clone())))
    });
    let relevant = Signal::derive(move || {
        dispatch.state.with(|s| {
            matches!(
                s.command(),
                Some(Command::MetadataState { .. } | Command::MetadataCommand { .. })
            )
        })
    });
    let can_publish = Signal::derive(move || {
        !blocked.get()
            && !archived.get()
            && !unsaved.get()
            && confirmed.get()
            && review.with(|r| {
                r.as_ref().is_some_and(|r| {
                    current.with(|s| {
                        s.as_ref().is_some_and(|s| {
                            s.work_id == r.work() && s.version == r.expected_version()
                        })
                    })
                })
            })
    });
    let read = Callback::new(move |_| {
        if blocked.get_untracked() {
            return;
        }
        if let Some(work) = facts.with_untracked(|f| f.work.as_ref().map(|w| w.id.clone())) {
            facts.update(|f| f.metadata.current = None);
            dispatch.send(Command::MetadataState { work });
        }
    });
    let close = Callback::new(move |_| {
        review.set(None);
        confirmed.set(false);
        selection_error.set(false);
        request_animation_frame(move || focus("metadata-review-open"));
    });
    let status = Signal::derive(move || {
        if relevant.get() {
            match dispatch.state.with(|s| s.phase()) {
                Phase::Pending => return Key::MetadataPending,
                Phase::Ambiguous => return Key::MetadataUnknown,
                Phase::Failed => {
                    return if dispatch.state.with(|s| {
                        s.error().is_some_and(|e| {
                            matches!(e, super::ClientError::Http { status: 409, .. })
                        })
                    }) {
                        Key::MetadataConflict
                    } else {
                        Key::MetadataFailed
                    }
                }
                Phase::Idle | Phase::Ready => {}
            }
        }
        if archived.get() {
            return Key::MetadataArchived;
        }
        current.with(|s| match s.as_ref().map(|s| s.lifecycle) {
            Some(Lifecycle::NeverPublished) => Key::MetadataNever,
            Some(Lifecycle::Active) => Key::MetadataActive,
            Some(Lifecycle::Withdrawn) => Key::MetadataWithdrawn,
            None => Key::MetadataUnread,
        })
    });
    view! {
      <section class="cw-metadata cw-block" aria-labelledby="metadata-title" hidden=move || !owner.get()>
        <h5 id="metadata-title">{move || text(Key::MetadataPublication, locale.get())}</h5>
        <div class="cw-publication-strip" aria-live="polite">
          <p id="metadata-status" role="status">{move || text(status.get(), locale.get())}</p>
          <p>{move || text(Key::MetadataSaved, locale.get())}" "{move || facts.with(|f| f.work.as_ref().map(|w| super::copy::revision_label(w.revision)))}</p>
          <p hidden=move || current.with(|s| s.as_ref().and_then(|s| s.generation.as_ref()).is_none())>{move || text(Key::MetadataHistory, locale.get())}" "{move || current.with(|s| s.as_ref().and_then(|s| s.generation.as_ref()).map(|g| super::copy::revision_label(g.work_revision)))}</p>
          <p hidden=move || current.with(Option::is_none)>{move || text(Key::MetadataVersion, locale.get())}" "{move || current.with(|s| s.as_ref().map(|s| s.version))}</p>
        </div>
        <p role="status" class="cw-reason" hidden=move || !unsaved.get()>{move || text(Key::MetadataDirty, locale.get())}</p>
        <div class="cw-actions">
          <ActionButton id="metadata-read" locale blocked on_press=read variant=ButtonVariant::Text>{move || text(Key::MetadataLoad, locale.get())}</ActionButton>
          <ActionButton id="metadata-review-open" locale reason=BlockReason::Metadata(Key::MetadataReviewBlocked) blocked=Signal::derive(move || blocked.get() || archived.get() || unsaved.get() || current.with(Option::is_none) || review.with(Option::is_some)) variant=ButtonVariant::Tonal on_press=Callback::new(move |_| {
              if blocked.get_untracked() || unsaved.get_untracked() { return; }
              let candidate = facts.with_untracked(|f| MetadataReview::new(f.work.as_ref()?, f.metadata.current.as_ref()?).ok());
              if let Some(candidate) = candidate {
                  let command=Command::MetadataArtwork{work:candidate.work().into(),revision:candidate.work_revision()};
                  review.set(Some(candidate)); confirmed.set(false); selection_error.set(false); dispatch.send(command);
                  request_animation_frame(move || focus("metadata-review-title"));
              }
          })>{move || text(Key::MetadataReview, locale.get())}</ActionButton>
          <ActionButton id="metadata-withdraw" locale reason=BlockReason::Metadata(Key::MetadataActiveRequired) blocked=Signal::derive(move || blocked.get() || archived.get() || current.with(|s| s.as_ref().is_none_or(|s| s.lifecycle != Lifecycle::Active))) variant=ButtonVariant::Text on_press=Callback::new(move |_| {
              if blocked.get_untracked() { return; }
              if let Some(state) = current.get_untracked().filter(|s| s.lifecycle == Lifecycle::Active) {
                  let prompt = format!("{}\n{} · v{} · {}", text(Key::MetadataConfirmWithdraw,locale.get_untracked()), state.work_id, state.version, state.generation.as_ref().map(|g| super::copy::revision_label(g.work_revision)).unwrap_or_default());
                  if web_sys::window().is_some_and(|w| w.confirm_with_message(&prompt).unwrap_or(false)) {
                      dispatch.build(uuid().map(|operation_id| Command::MetadataCommand { work: state.work_id, request: Request::Withdraw { operation_id, expected_version: state.version } }));
                  }
              }
          })>{move || text(Key::MetadataWithdraw, locale.get())}</ActionButton>
        </div>
        <div hidden=move || !(relevant.get() && dispatch.state.with(|s| s.can_retry()))>
          <ActionButton id="metadata-reconcile" locale blocked=Signal::derive(move || dispatch.session_pending.get()) on_press=Callback::new(move |_| dispatch.retry())>{move || text(Key::MetadataRetry, locale.get())}</ActionButton>
        </div>
        <p class="help" role="status" hidden=move || facts.with(|f| f.metadata.receipt.is_none())>{move || text(Key::MetadataReceipt, locale.get())}</p>
        <details hidden=move ||facts.with(|f|f.metadata.receipt.is_none())>
          <summary>{move ||text(Key::MetadataHistory,locale.get())}</summary>
          <code>{move ||facts.with(|f|f.metadata.receipt.as_ref().map(|s|super::copy::metadata_identity(&s.work_id,s.version,s.generation.as_ref().map_or(0,|g|g.work_revision))))}</code>
        </details>
        <div class="cw-metadata-audio" hidden=move ||facts.with(|f| f.unit.as_ref().and_then(|u| f.metadata.current.as_ref()?.generation.as_ref()?.entries.iter().find(|e|e.unit_id==u.id)).is_none())>
          <p>{move ||text(if facts.with(|f| f.unit.as_ref().is_some_and(|u| f.metadata.current.as_ref().and_then(|s|s.generation.as_ref()).and_then(|g|g.entries.iter().find(|e|e.unit_id==u.id)).is_some_and(|e|e.audio.is_some()))) {Key::MetadataAudioLinked} else if facts.with(|f|f.unit.as_ref().is_some_and(|u|f.publication.active.as_ref().and_then(|a|a.release.as_ref()).is_some_and(|r|r.manifest.unit_id==u.id))) {Key::MetadataAudioReady} else {Key::MetadataAudioNone},locale.get())}</p>
          <p><code>{move ||facts.with(|f|f.publication.active.as_ref().and_then(|a|a.release.as_ref()).map(|r|super::copy::revision_identity(r.manifest.unit_metadata_revision,&r.id)))}</code></p>
          <ReviewCheckbox id="metadata-display-only" locale checked=display_only label=Signal::derive(move ||text(Key::MetadataDisplayOnly,locale.get()).to_owned()) blocked on_change=Callback::new(move |value|display_only.set(value))/>
          <ActionButton id="metadata-associate" locale reason=BlockReason::Metadata(Key::MetadataAudioRequired) variant=ButtonVariant::Text blocked=Signal::derive(move ||blocked.get() ||archived.get() ||current.with(Option::is_none) || facts.with(|f|f.unit.as_ref().is_none_or(|u| f.publication.active.as_ref().and_then(|a|a.release.as_ref()).is_none_or(|r|r.manifest.unit_id!=u.id || f.metadata.current.as_ref().and_then(|s|s.generation.as_ref()).and_then(|g|g.entries.iter().find(|e|e.unit_id==u.id)).and_then(|e|e.audio.as_ref()).is_some_and(|a|a.release_id==r.id))))) on_press=Callback::new(move |_| {
            if blocked.get_untracked(){return;}
            let choice=facts.with_untracked(|f|{let u=f.unit.as_ref()?;let s=f.metadata.current.as_ref()?;let entry=s.generation.as_ref()?.entries.iter().find(|e|e.unit_id==u.id)?;let r=f.publication.active.as_ref()?.release.as_ref()?;if r.manifest.unit_id!=u.id{return None;}Some((s.clone(),entry.clone(),r.id.clone()))});
            if let Some((state,entry,release))=choice {
              let prompt=format!("{}\n{} · r{}\n{}",text(Key::MetadataAssociateConfirm,locale.get_untracked()),entry.unit_id,entry.revision,release);
              if web_sys::window().is_some_and(|w|w.confirm_with_message(&prompt).unwrap_or(false)) {
                dispatch.build(uuid().map(|operation_id|Command::MetadataCommand{work:state.work_id,request:Request::AssociateAudio{operation_id,expected_version:state.version,unit_id:entry.unit_id,release_id:release,confirm_display_only_drift:display_only.get_untracked()}}));
              }
            }
          })>{move ||text(Key::MetadataAssociate,locale.get())}</ActionButton>
        </div>
        <Show when=move || review.with(Option::is_some)>
          <section id="metadata-review" class="cw-metadata-review" aria-labelledby="metadata-review-title">
            <h6 id="metadata-review-title" tabindex="-1">{move || text(Key::MetadataSelection, locale.get())}</h6>
            <p class="cw-metadata-pin">{move || review.with(|r| r.as_ref().map(|r| super::copy::metadata_identity(r.work(),r.expected_version(),r.work_revision())))}</p>
            <p>{move || text(Key::MetadataPublicAudience, locale.get())}</p>
            <h6>{move ||text(Key::MetadataWorkText,locale.get())}</h6>
            <p>{move ||review.with(|r|r.as_ref().map(|r|r.saved().details.title.clone()))}</p>
            <p>{move ||review.with(|r|r.as_ref().map(|r|r.saved().details.synopsis.clone()))}</p>
            <p class="help">{move ||text(Key::MetadataDiff,locale.get())}</p>
            <p><code>{move ||review.with(|r|r.as_ref().map(|r|super::copy::revision_transition(comparison.with(|s|s.as_ref().and_then(|s|s.generation.as_ref()).map(|g|g.work_revision)),r.work_revision())))}</code></p>
            <p class="help">{move || text(Key::MetadataNoAutoSelect, locale.get())}</p>
            <div class="cw-metadata-candidates">
              <For each=move || facts.with(|f| f.units.iter().filter(|u| u.lifecycle == "draft").cloned().collect::<Vec<_>>()) key=|u| (u.id.clone(), u.revision) children=move |unit| {
                  let unit = StoredValue::new(unit);
                  view! { <div class="cw-metadata-candidate"><span>{move || unit.with_value(|u| super::copy::labelled_revision(&u.details.title,u.revision))}</span>
                    <ActionButton id=unit.with_value(|u| format!("metadata-select-{}",u.id)) locale variant=ButtonVariant::Text blocked on_press=Callback::new(move |_| {
                        if blocked.get_untracked() { return; }
                        review.update(|r| { if let Some(old) = r { match unit.with_value(|u| old.select(u)) { Ok(next) => { *old = next; confirmed.set(false); selection_error.set(false); }, Err(_) => selection_error.set(true) } } });
                    })>{move || text(Key::MetadataSelectSaved, locale.get())}</ActionButton></div> }
              }/>
            </div>
            <div class="cw-actions" hidden=move ||review.with(|r|r.as_ref().is_none_or(|r|current.with(|s|s.as_ref().is_none_or(|s|s.version==r.expected_version()))))>
              <ActionButton id="metadata-rebase" locale blocked variant=ButtonVariant::Text on_press=Callback::new(move |_| {
                if blocked.get_untracked(){return;}
                if let Some(state)=current.get_untracked(){review.update(|r|if let Some(old)=r {if let Ok(next)=old.rebase(&state){*old=next;confirmed.set(false);}});}
              })>{move ||text(Key::MetadataRebase,locale.get())}</ActionButton>
            </div>
            <p class="help">{move || text(Key::MetadataBounds, locale.get())}</p>
            <p role="alert" hidden=move || !selection_error.get()>{move ||text(Key::MetadataBounds,locale.get())}</p>
            <ol id="metadata-final-order" class="cw-metadata-order">
              <For each=move || review.with(|r| r.as_ref().map(|r| r.units().to_vec()).unwrap_or_default()) key=|u| (u.unit_id.clone(), u.revision) children=move |unit| {
                  let unit = StoredValue::new(unit);
                  let associated = Signal::derive(move || comparison.with(|s| s.as_ref().and_then(|s|s.generation.as_ref()).and_then(|g|g.entries.iter().find(|e| unit.with_value(|u|e.unit_id==u.unit_id && e.revision==u.revision))).and_then(|e|e.audio.clone())));
                  view! { <li><p>{move || facts.with(|f| f.units.iter().find(|candidate| unit.with_value(|u| candidate.id==u.unit_id && candidate.revision==u.revision)).map(|u|u.details.title.clone())).unwrap_or_else(||text(Key::MetadataOldTextUnavailable,locale.get()).into())}</p><code>{move || unit.with_value(|u| super::copy::revision_identity(u.revision,&u.unit_id))}</code>
                    <p class="help"><code>{move ||review.with(|r|r.as_ref().and_then(|r|r.units().iter().position(|e|unit.with_value(|u|e.unit_id==u.unit_id))).map(|i|super::copy::metadata_order_transition(comparison.with(|s|s.as_ref().and_then(|s|s.generation.as_ref()).and_then(|g|g.entries.iter().find(|e|unit.with_value(|u|e.unit_id==u.unit_id))).map(|e|(e.position,e.revision))),(i+1,unit.with_value(|u|u.revision)))))}</code></p>
                    <p class="help">{move || text(if associated.with(Option::is_some) {Key::MetadataAudioLinked} else {Key::MetadataAudioNone},locale.get())}</p>
                    <div class="cw-actions">
                      <ActionButton id=unit.with_value(|u|format!("metadata-up-{}",u.unit_id)) locale variant=ButtonVariant::Text icon=IconName::ArrowUpward blocked on_press=Callback::new(move |_| { if blocked.get_untracked(){return;} review.update(|r|if let Some(old)=r { if let Ok(next)=unit.with_value(|u|old.move_unit(&u.unit_id,true)){*old=next;confirmed.set(false);} }); })>{move || text(Key::MoveEarlier, locale.get())}</ActionButton>
                      <ActionButton id=unit.with_value(|u|format!("metadata-down-{}",u.unit_id)) locale variant=ButtonVariant::Text icon=IconName::ArrowDownward blocked on_press=Callback::new(move |_| { if blocked.get_untracked(){return;} review.update(|r|if let Some(old)=r {if let Ok(next)=unit.with_value(|u|old.move_unit(&u.unit_id,false)){*old=next;confirmed.set(false);} }); })>{move || text(Key::MoveLater, locale.get())}</ActionButton>
                      <ActionButton id=unit.with_value(|u|format!("metadata-remove-{}",u.unit_id)) locale variant=ButtonVariant::Text blocked on_press=Callback::new(move |_| { if blocked.get_untracked(){return;} review.update(|r|if let Some(old)=r {*old=unit.with_value(|u|old.remove(&u.unit_id));confirmed.set(false);}); })>{move || text(Key::MetadataRemove, locale.get())}</ActionButton>
                    </div></li> }
              }/>
            </ol>
            <h6>{move ||text(Key::MetadataArtwork,locale.get())}</h6>
            <For each=move ||review.with(|r|r.as_ref().map(|r|{
                let mut revisions=vec![r.work_revision()];
                if let Some(old)=comparison.with(|s|s.as_ref().and_then(|s|s.generation.as_ref()).map(|g|g.work_revision)){if old!=r.work_revision(){revisions.insert(0,old);}}
                revisions
            }).unwrap_or_default()) key=|r|*r children=move |revision| view!{
                <div class="cw-metadata-asset">
                  <p><code>{super::copy::revision_label(revision)}</code></p>
                  <p>{move ||facts.with(|f|match f.metadata.artwork.get(&revision){None=>text(Key::MetadataArtworkUnread,locale.get()).into(),Some(pin)=>pin.artwork.as_ref().map(|a|a.alt_text.clone()).unwrap_or_else(||text(Key::MetadataArtworkNone,locale.get()).into())})}</p>
                  <code>{move ||facts.with(|f|f.metadata.artwork.get(&revision).and_then(|p|p.artwork.as_ref()).map(|a|super::copy::asset_identity(&a.image_asset_id,&a.image_asset_digest)))}</code>
                  <ActionButton id=format!("metadata-artwork-{revision}") locale blocked variant=ButtonVariant::Text on_press=Callback::new(move |_| {
                      if blocked.get_untracked() { return; }
                      if let Some(selection) = review.get_untracked() {
                          dispatch.send(Command::MetadataArtwork { work: selection.work().into(), revision });
                      }
                  })>{move ||text(Key::MetadataArtworkRead,locale.get())}</ActionButton>
                </div>
            }/>
            <p class="help">{move || text(Key::MetadataPinAssets, locale.get())}</p>
            <ReviewCheckbox id="metadata-rights" locale label=Signal::derive(move ||text(Key::MetadataRights,locale.get()).to_owned()) checked=confirmed blocked on_change=Callback::new(move |value|confirmed.set(value))/>
            <div class="cw-actions">
              <ActionButton id="metadata-publish" locale reason=BlockReason::Metadata(Key::MetadataPublishBlocked) variant=ButtonVariant::Primary blocked=Signal::derive(move || !can_publish.get()) on_press=Callback::new(move |_| {
                  if !can_publish.get_untracked(){return;}
                  if let Some(selection)=review.get_untracked(){dispatch.build(uuid().and_then(|id|selection.request(id,confirmed.get_untracked()).map(|request|Command::MetadataCommand{work:selection.work().into(),request}).map_err(|_|super::ClientError::InvalidInput)));}
              })>{move ||text(Key::MetadataPublish,locale.get())}</ActionButton>
              <ActionButton id="metadata-review-close" locale blocked variant=ButtonVariant::Text on_press=close>{move || text(Key::MetadataClose,locale.get())}</ActionButton>
            </div>
          </section>
        </Show>
      </section>
    }
}
