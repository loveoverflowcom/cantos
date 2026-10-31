//! Studio-owned matrix composition over the existing field, Select, review and button owners.
use super::{chapter_matrix::UnitPolicyReview, Command, Facts, Phase, RequestState};
use crate::{
    contract::*,
    localization::{
        generated::live::{text, Key},
        Locale,
    },
    ui::{ActionButton, ReviewCheckbox},
};
use cantos_ui::{ButtonVariant, FieldDensity, Select, SelectOption, TextField, TextFieldVariant};
use leptos::prelude::*;

#[component]
pub(super) fn ChapterPanel(
    facts: RwSignal<Facts>,
    state: RwSignal<RequestState>,
    locale: Signal<Locale>,
    blocked: Signal<bool>,
    dirty: RwSignal<bool>,
    on_send: Callback<Command>,
    on_retry: Callback<()>,
) -> impl IntoView {
    let review = RwSignal::new(None::<UnitPolicyReview>);
    let actor = RwSignal::new(String::new());
    let group = RwSignal::new(String::new());
    let until = RwSignal::new(String::new());
    let confirmed = RwSignal::new(false);
    let publication_confirmed = RwSignal::new(false);
    let position = RwSignal::new(String::new());
    let invalid = RwSignal::new(false);
    let acknowledged = StoredValue::new(String::new());
    let context = Signal::derive(move || {
        (
            facts.with(|f| f.work.as_ref().map(|w| w.id.clone())),
            state.with(|s| s.actor().to_owned()),
        )
    });
    Effect::new(move |old: Option<(Option<String>, String)>| {
        let current = context.get();
        if old.as_ref().is_some_and(|old| old != &current) {
            review.set(None);
            actor.set(String::new());
            group.set(String::new());
            until.set(String::new());
            confirmed.set(false);
            invalid.set(false);
            publication_confirmed.set(false);
            position.set(String::new());
        }
        current
    });
    Effect::new(move |_| {
        dirty.set(
            review.with(|r| r.as_ref().is_some_and(UnitPolicyReview::dirty))
                || !actor.get().is_empty()
                || !until.get().is_empty()
                || !group.get().is_empty()
                || !position.get().is_empty()
                || publication_confirmed.get(),
        )
    });
    on_cleanup(move || {
        let _ = dirty.try_set(false);
    });
    Effect::new(move |_| {
        let command = state.with(|s| {
            if s.phase() == Phase::Ready {
                s.command().cloned()
            } else {
                None
            }
        });
        if let Some(Command::ViewerChange {
            work,
            request:
                ViewerAccessRequest {
                    operation_id,
                    change:
                        ViewerAccessChange::SetUnitPolicy {
                            unit_id, policy, ..
                        },
                    ..
                },
        }) = command
        {
            if acknowledged.with_value(|old| old != &operation_id) {
                acknowledged.set_value(operation_id);
                review.update(|r| {
                    if let Some(r) = r {
                        if r.work == work && r.row.unit_id == unit_id {
                            r.row.policy = policy;
                        }
                    }
                });
                confirmed.set(false);
            }
        }
    });
    let page = Signal::derive(move || facts.with(|f| f.viewer.matrix.clone()));
    let unavailable = Signal::derive(move || {
        blocked.get()
            || facts.with(|f| {
                f.work.as_ref().is_none_or(|w| {
                    state.with(|s| w.owner_id != s.actor()) || w.lifecycle == "archived"
                })
            })
    });
    let stale = Signal::derive(move || {
        review.with(|r| {
            r.as_ref().is_some_and(|r| {
                page.with(|p| {
                    p.as_ref().is_none_or(|p| {
                        p.work_id != r.work
                            || p.policy_version != r.policy_version
                            || p.publication_version != r.publication_version
                            || !p.items.iter().any(|u| u.unit_id == r.row.unit_id)
                    })
                })
            })
        })
    });
    let load = Callback::new(move |cursor: Option<String>| {
        if unavailable.get_untracked() {
            return;
        }
        if let Some(work) = facts.with_untracked(|f| f.work.as_ref().map(|w| w.id.clone())) {
            facts.update(|f| f.viewer.matrix = None);
            confirmed.set(false);
            publication_confirmed.set(false);
            on_send.run(Command::ChapterMatrix { work, cursor });
        }
    });
    let add = Callback::new(move |principal: ViewerPrincipal| {
        if unavailable.get_untracked() {
            return;
        }
        match super::viewer_panel::expiry(&until.get_untracked()) {
            Ok(expires_at_us) => {
                let next = review.with_untracked(|r| {
                    r.as_ref().and_then(|r| {
                        r.add(UnitViewerGrant {
                            principal,
                            expires_at_us,
                        })
                    })
                });
                if let Some(next) = next {
                    review.set(Some(next));
                    actor.set(String::new());
                    group.set(String::new());
                    until.set(String::new());
                    confirmed.set(false);
                    invalid.set(false);
                } else {
                    invalid.set(true);
                }
            }
            Err(()) => invalid.set(true),
        }
    });
    let publish = Callback::new(move |withdraw: bool| {
        if unavailable.get_untracked() {
            return;
        }
        if let Some(operation_id) = super::viewer_panel::operation() {
            let request = review.with_untracked(|r| {
                r.as_ref().and_then(|r| {
                    page.with_untracked(|p| {
                        p.as_ref().and_then(|p| {
                            r.publication_request(
                                p,
                                operation_id,
                                position.with_untracked(|p| p.parse().ok()),
                                withdraw,
                                publication_confirmed.get_untracked(),
                            )
                        })
                    })
                })
            });
            if let Some(request) = request {
                publication_confirmed.set(false);
                on_send.run(Command::MetadataCommand {
                    work: review
                        .with_untracked(|r| r.as_ref().map(|r| r.work.clone()).unwrap_or_default()),
                    request,
                });
            }
        }
    });
    view! {
      <section id="chapter-matrix" class="cw-block" aria-labelledby="chapter-matrix-title">
        <h5 id="chapter-matrix-title">{move ||text(Key::ChapterMatrix,locale.get())}</h5>
        <div class="cw-actions">
          <ActionButton id="chapter-matrix-load" locale blocked=unavailable on_press=Callback::new(move |_|load.run(None))>{move ||text(Key::ChapterMatrixLoad,locale.get())}</ActionButton>
          <ActionButton id="chapter-matrix-next" locale variant=ButtonVariant::Text blocked=Signal::derive(move ||unavailable.get()||page.with(|p|p.as_ref().is_none_or(|p|p.next_cursor.is_none()))) on_press=Callback::new(move |_|{if let Some(cursor)=page.with_untracked(|p|p.as_ref().and_then(|p|p.next_cursor.clone())){load.run(Some(cursor));}})>{move ||text(Key::ChapterMatrixNext,locale.get())}</ActionButton>
          <Show when=move ||state.with(|s|matches!(s.command(),Some(Command::ChapterMatrix {..}|Command::ViewerChange {..}|Command::MetadataCommand {..}))&&s.can_retry())><ActionButton id="chapter-matrix-retry" locale blocked=Signal::derive(move ||!state.with(|s|s.can_retry())) on_press=Callback::new(move |_|on_retry.run(()))>{move ||text(Key::AccessUnknown,locale.get())}</ActionButton></Show>
        </div>
        <p role="status" hidden=move ||page.with(Option::is_some)>{move ||text(Key::ChapterMatrixUnread,locale.get())}</p>
        <p>{move ||page.with(|p|p.as_ref().map(|p|text(if p.work_audience==ViewerAudience::Public {Key::AccessPublic}else{Key::AccessRestricted},locale.get())))}</p>
        <p>{move ||text(Key::ChapterPolicyHelp,locale.get())}</p>
        <ul class="cw-metadata-order">
          <For each=move ||page.with(|p|p.as_ref().map(|p|p.items.iter().map(|r|r.unit_id.clone()).collect::<Vec<_>>()).unwrap_or_default()) key=|id|id.clone() children=move |id| {
            let id=StoredValue::new(id);
            let row=Signal::derive(move ||page.with(|p|p.as_ref().and_then(|p|p.items.iter().find(|r|id.with_value(|id|id==&r.unit_id)).cloned())));
            view! {<li>
              <h6>{move ||row.with(|r|r.as_ref().map(|r|r.title.clone()))}</h6>
              <p>{move ||text(Key::ChapterMatrixDraft,locale.get())}" "{move ||row.with(|r|r.as_ref().map(|r|super::copy::revision_label(r.draft_revision)))}</p>
              <p>{move ||text(Key::ChapterMatrixPublished,locale.get())}" "{move ||row.with(|r|r.as_ref().map(|r|r.published_revision.map(super::copy::revision_label).unwrap_or_else(||text(Key::ChapterMatrixUnpublished,locale.get()).into())))}</p>
              <p>{move ||row.with(|r|r.as_ref().map(|r|text(if matches!(r.policy,UnitViewerPolicy::Inherit){Key::ChapterPolicyInherit}else{Key::ChapterPolicyRestricted},locale.get())))}</p>
              <p>{move ||row.with(|r|r.as_ref().map(|r|text(if r.audio.is_some(){Key::ChapterMatrixAssociated}else{Key::ChapterMatrixNoAudio},locale.get())))}</p>
              <ActionButton id=id.with_value(|id|format!("chapter-edit-{id}")) locale variant=ButtonVariant::Text blocked=Signal::derive(move ||unavailable.get()||dirty.get()) on_press=Callback::new(move |_| {
                if unavailable.get_untracked()||dirty.get_untracked(){return;}
                let next=page.with_untracked(|p|p.as_ref().and_then(|p|id.with_value(|id|UnitPolicyReview::select(p,id))));review.set(next);confirmed.set(false);invalid.set(false);
              })>{move ||text(Key::ChapterMatrixEdit,locale.get())}</ActionButton>
            </li>}
          }/>
        </ul>
        <Show when=move ||review.with(Option::is_some)>
          <section class="cw-metadata-review" aria-labelledby="chapter-policy-title">
            <h6 id="chapter-policy-title">{move ||review.with(|r|r.as_ref().map(|r|r.row.title.clone()))}</h6>
            <code>{move ||review.with(|r|r.as_ref().map(|r|super::copy::revision_identity(r.row.draft_revision,&r.row.unit_id)))}</code>
            <Select id="chapter-policy-mode" label=Signal::derive(move ||text(Key::ChapterPolicyMode,locale.get()).to_owned()) value=Signal::derive(move ||review.with(|r|if r.as_ref().is_some_and(|r|matches!(r.policy,UnitViewerPolicy::Inherit)){"inherit"}else{"restricted"}).to_owned()) options=Signal::derive(move ||vec![SelectOption::new("inherit",text(Key::ChapterPolicyInherit,locale.get())),SelectOption::new("restricted",text(Key::ChapterPolicyRestricted,locale.get()))]) on_input=Callback::new(move |value:String| {
              if unavailable.get_untracked(){return;}review.update(|r|if let Some(r)=r {match value.as_str(){"inherit"=>r.policy=UnitViewerPolicy::Inherit,"restricted" if matches!(r.policy,UnitViewerPolicy::Inherit)=>r.policy=UnitViewerPolicy::Restricted {grants:vec![]},_=>()}});confirmed.set(false);
            })/>
            <p class="cw-reason" hidden=move ||!review.with(|r|r.as_ref().is_some_and(|r|matches!(&r.policy,UnitViewerPolicy::Restricted {grants} if grants.is_empty())))>{move ||text(Key::ChapterPolicyEmpty,locale.get())}</p>
            <ul><For each=move ||review.with(|r|match r.as_ref().map(|r|&r.policy){Some(UnitViewerPolicy::Restricted {grants})=>grants.clone(),_=>vec![]}) key=|g|match &g.principal {ViewerPrincipal::Actor {actor_id}=>format!("actor:{actor_id}"),ViewerPrincipal::Group {group_id}=>format!("group:{group_id}")} children=move |grant| {
              let principal=StoredValue::new(grant.principal);
              view! {<li><span>{move ||principal.with_value(|p|match p {ViewerPrincipal::Actor {actor_id}=>actor_id.clone(),ViewerPrincipal::Group {group_id}=>group_id.clone()})}</span>
                <ActionButton id=principal.with_value(|p|match p {ViewerPrincipal::Actor {actor_id}=>format!("chapter-remove-actor-{actor_id}"),ViewerPrincipal::Group {group_id}=>format!("chapter-remove-group-{group_id}")}) locale variant=ButtonVariant::Text blocked=unavailable on_press=Callback::new(move |_|{if unavailable.get_untracked(){return;}review.update(|r|if let Some(old)=r {*old=principal.with_value(|p|old.remove(p));});confirmed.set(false);})>{move ||text(Key::ChapterPolicyRemove,locale.get())}</ActionButton>
              </li>}
            }/></ul>
            <TextField id="chapter-viewer-actor" label=Signal::derive(move ||text(Key::AccessActor,locale.get()).to_owned()) value=actor on_input=Callback::new(move |v|{actor.set(v);confirmed.set(false);}) variant=TextFieldVariant::Floating density=FieldDensity::Compact/>
            <TextField id="chapter-viewer-expiry" label=Signal::derive(move ||text(Key::AccessExpiryInput,locale.get()).to_owned()) value=until on_input=Callback::new(move |v|{until.set(v);confirmed.set(false);}) variant=TextFieldVariant::Floating density=FieldDensity::Compact/>
            <ActionButton id="chapter-add-actor" locale blocked=Signal::derive(move ||unavailable.get()||actor.with(|a|a.is_empty()||a.len()>64||!a.bytes().all(|b|b.is_ascii_lowercase()||b.is_ascii_digit()||b==b'_'||b==b'-'))) on_press=Callback::new(move |_|add.run(ViewerPrincipal::Actor {actor_id:actor.get_untracked()}))>{move ||text(Key::ChapterPolicyAddActor,locale.get())}</ActionButton>
            <Select id="chapter-viewer-group" label=Signal::derive(move ||text(Key::AccessGroupGrant,locale.get()).to_owned()) value=group options=Signal::derive(move ||{
              let mut rows=vec![SelectOption::new("",text(Key::AccessGroupGrant,locale.get()))];
              facts.with(|f|if let Some(groups)=&f.viewer.groups {rows.extend(groups.items.iter().map(|g|SelectOption::new(g.group_id.clone(),g.name.clone())));});rows
            }) on_input=Callback::new(move |v|{group.set(v);confirmed.set(false);})/>
            <ActionButton id="chapter-add-group" locale blocked=Signal::derive(move ||unavailable.get()||group.with(String::is_empty)) on_press=Callback::new(move |_|add.run(ViewerPrincipal::Group {group_id:group.get_untracked()}))>{move ||text(Key::ChapterPolicyAddGroup,locale.get())}</ActionButton>
            <p role="alert" hidden=move ||!invalid.get()>{move ||text(Key::ChapterPolicyInvalid,locale.get())}</p>
            <p role="status" hidden=move ||!stale.get()>{move ||text(Key::ChapterPolicyConflict,locale.get())}</p>
            <ActionButton id="chapter-policy-compare" locale blocked=Signal::derive(move ||unavailable.get()||page.with(Option::is_none)) on_press=Callback::new(move |_|{
              if unavailable.get_untracked(){return;}let next=review.with_untracked(|r|r.as_ref().and_then(|r|page.with_untracked(|p|p.as_ref().and_then(|p|r.rebase(p)))));if let Some(next)=next{review.set(Some(next));confirmed.set(false);}
            })>{move ||text(Key::ChapterPolicyCompare,locale.get())}</ActionButton>
            <ReviewCheckbox id="chapter-policy-confirm" locale label=Signal::derive(move ||text(Key::ChapterPolicyConfirm,locale.get()).to_owned()) checked=confirmed on_change=Callback::new(move |v|confirmed.set(v))/>
            <ActionButton id="chapter-policy-apply" locale blocked=Signal::derive(move ||unavailable.get()||stale.get()||!confirmed.get()||!review.with(|r|r.as_ref().is_some_and(UnitPolicyReview::dirty))) on_press=Callback::new(move |_|{
              if unavailable.get_untracked(){return;}
              if let Some(operation_id)=super::viewer_panel::operation(){if let Some(request)=review.with_untracked(|r|r.as_ref().and_then(|r|page.with_untracked(|p|p.as_ref().and_then(|p|r.request(p,operation_id,confirmed.get_untracked()))))){confirmed.set(false);on_send.run(Command::ViewerChange {work:review.with_untracked(|r|r.as_ref().map(|r|r.work.clone()).unwrap_or_default()),request});}}
            })>{move ||text(Key::ChapterPolicyApply,locale.get())}</ActionButton>
            <p class="cw-reason" hidden=move ||page.with(|p|p.as_ref().is_some_and(|p|p.publication_lifecycle==MetadataPublicationLifecycle::Active))>{move ||text(Key::ChapterWorkActiveRequired,locale.get())}</p>
            <TextField id="chapter-publish-position" label=Signal::derive(move ||text(Key::ChapterPublishPosition,locale.get()).to_owned()) value=position on_input=Callback::new(move |v|{position.set(v);publication_confirmed.set(false);}) variant=TextFieldVariant::Floating density=FieldDensity::Compact/>
            <ReviewCheckbox id="chapter-publication-confirm" locale label=Signal::derive(move ||text(Key::ChapterPublicationConfirm,locale.get()).to_owned()) checked=publication_confirmed on_change=Callback::new(move |v|publication_confirmed.set(v))/>
            <ActionButton id="chapter-publish-saved" locale blocked=Signal::derive(move ||unavailable.get()||!publication_confirmed.get()||page.with(|p|p.as_ref().is_none_or(|p|p.publication_lifecycle!=MetadataPublicationLifecycle::Active))||position.with(|p|p.parse::<u32>().ok().is_none_or(|p|!(1..=1000).contains(&p)))) on_press=Callback::new(move |_|publish.run(false))>{move ||text(Key::ChapterPublishSaved,locale.get())}</ActionButton>
            <ActionButton id="chapter-withdraw" locale variant=ButtonVariant::Text blocked=Signal::derive(move ||unavailable.get()||!publication_confirmed.get()||review.with(|r|r.as_ref().is_none_or(|r|r.row.published_revision.is_none()))) on_press=Callback::new(move |_|publish.run(true))>{move ||text(Key::ChapterWithdraw,locale.get())}</ActionButton>
            <ActionButton id="chapter-policy-discard" locale variant=ButtonVariant::Text blocked=unavailable on_press=Callback::new(move |_|{if unavailable.get_untracked(){return;}review.set(None);actor.set(String::new());group.set(String::new());until.set(String::new());confirmed.set(false);publication_confirmed.set(false);position.set(String::new());invalid.set(false);})>{move ||text(Key::ChapterPolicyDiscard,locale.get())}</ActionButton>
          </section>
        </Show>
      </section>
    }
}
