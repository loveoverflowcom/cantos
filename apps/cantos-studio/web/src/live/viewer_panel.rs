//! App-owned access composition over admitted fields, buttons and Dialog. No ACL is computed here.
use super::{viewer_access::AudienceReview, Command, Facts, Phase, RequestState};
use crate::{
    contract::*,
    localization::{
        generated::live::{text, Key},
        Locale,
    },
    ui::{ActionButton, BlockReason, ReviewCheckbox},
};
use cantos_ui::{Button, ButtonVariant, Dialog, FieldDensity, TextField, TextFieldVariant};
use leptos::prelude::*;

pub(super) fn operation() -> Option<String> {
    #[cfg(target_arch = "wasm32")]
    {
        super::workspace::uuid().ok()
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        None
    }
}
pub(super) fn expiry(value: &str) -> Result<Option<i64>, ()> {
    if value.is_empty() {
        return Ok(None);
    }
    #[cfg(target_arch = "wasm32")]
    {
        let time = value.split_once('T').ok_or(())?.1;
        if !time.ends_with('Z') && !time.contains('+') && !time.contains('-') {
            return Err(());
        }
        let ms = js_sys::Date::parse(value);
        if ms.is_finite() && ms > 0.0 && ms < (i64::MAX / 1000) as f64 {
            return Ok(Some((ms as i64) * 1000));
        }
    }
    Err(())
}
fn expiry_label(value: Option<i64>, locale: Locale) -> String {
    match value {
        None => text(Key::AccessNoExpiry, locale).into(),
        Some(us) => crate::localization::present(
            cantos_localization::Formatter::try_new(locale.lang())
                .and_then(|f| f.date_time(us / 1000, cantos_localization::TimeZone::Utc)),
            locale,
        ),
    }
}
#[component]
fn Field(
    id: &'static str,
    label: Key,
    value: RwSignal<String>,
    locale: Signal<Locale>,
) -> impl IntoView {
    view! { <TextField id label=Signal::derive(move || text(label, locale.get()).to_owned()) value=Signal::derive(move || value.get()) on_input=Callback::new(move |v| value.set(v)) variant=TextFieldVariant::Floating density=FieldDensity::Compact/> }
}

#[component]
pub(super) fn ViewerPanel(
    facts: RwSignal<Facts>,
    state: RwSignal<RequestState>,
    locale: Signal<Locale>,
    blocked: Signal<bool>,
    dirty: RwSignal<bool>,
    on_send: Callback<Command>,
    on_retry: Callback<()>,
) -> impl IntoView {
    let open = RwSignal::new(false);
    let chapter_dirty = RwSignal::new(false);
    let email = RwSignal::new(String::new());
    let actor = RwSignal::new(String::new());
    let until = RwSignal::new(String::new());
    let group_name = RwSignal::new(String::new());
    let invalid_expiry = RwSignal::new(false);
    let review = RwSignal::new(None::<AudienceReview>);
    let confirmation = RwSignal::new(None::<Command>);
    let confirmed = RwSignal::new(false);
    let dialog_open = RwSignal::new(false);
    let dialog_error = RwSignal::new(false);
    let owner = Memo::new(move |_| {
        facts.with(|f| {
            f.work
                .as_ref()
                .is_some_and(|w| state.with(|s| w.owner_id == s.actor()))
        })
    });
    let context = Memo::new(move |_| {
        (
            facts.with(|f| f.work.as_ref().map(|w| w.id.clone())),
            state.with(|s| s.actor().to_owned()),
        )
    });
    Effect::new(move |old: Option<(Option<String>, String)>| {
        let current = context.get();
        if old.as_ref().is_some_and(|old| old != &current) {
            for field in [email, actor, until, group_name] {
                field.set(String::new());
            }
            review.set(None);
            confirmation.set(None);
            dialog_open.set(false);
            open.set(false);
        }
        current
    });
    Effect::new(move |_| {
        dirty.set(
            [email.get(), actor.get(), until.get(), group_name.get()]
                .iter()
                .any(|s| !s.is_empty())
                || dialog_open.get()
                || chapter_dirty.get(),
        )
    });
    Effect::new(move |_| {
        let completed = state.with(|s| {
            if s.phase() == Phase::Ready {
                s.command().cloned()
            } else {
                None
            }
        });
        match completed {
            Some(Command::ViewerChange { request, .. }) => match request.change {
                ViewerAccessChange::ShareMailbox {
                    email: submitted, ..
                } if email.get_untracked() == submitted => email.set(String::new()),
                ViewerAccessChange::GrantActor { actor_id, .. }
                    if actor.get_untracked() == actor_id =>
                {
                    actor.set(String::new())
                }
                _ => (),
            },
            Some(Command::ViewerGroupChange {
                request:
                    ViewerGroupRequest {
                        change: ViewerGroupChange::Create { name },
                        ..
                    },
                ..
            }) if group_name.get_untracked() == name => group_name.set(String::new()),
            _ => (),
        }
    });
    let current = Signal::derive(move || facts.with(|f| f.viewer.current.clone()));
    let can_change =
        Signal::derive(move || owner.get() && !blocked.get() && current.with(Option::is_some));
    let prepare = Callback::new(move |command: Command| {
        if blocked.get_untracked() || !owner.get_untracked() {
            return;
        }
        if !matches!(
            &command,
            Command::ViewerChange {
                request: ViewerAccessRequest {
                    change: ViewerAccessChange::SetAudience { .. },
                    ..
                },
                ..
            }
        ) {
            review.set(None);
        }
        confirmation.set(Some(command));
        confirmed.set(false);
        dialog_open.set(true);
    });
    let change = Callback::new(move |value: ViewerAccessChange| {
        if !can_change.get_untracked() {
            return;
        }
        if let (Some(current), Some(operation_id)) = (current.get_untracked(), operation()) {
            prepare.run(Command::ViewerChange {
                work: current.work_id,
                request: ViewerAccessRequest {
                    operation_id,
                    expected_version: current.version,
                    change: value,
                },
            });
        }
    });
    let relevant = Signal::derive(move || {
        state.with(|s| {
            matches!(
                s.command(),
                Some(
                    Command::ViewerState { .. }
                        | Command::ViewerChange { .. }
                        | Command::ViewerGroups
                        | Command::ViewerGroupChange { .. }
                )
            )
        })
    });
    let status = Signal::derive(move || {
        if relevant.get() {
            match state.with(|s| s.phase()) {
                Phase::Pending => return Key::AccessLoading,
                Phase::Ambiguous => return Key::AccessUnknown,
                Phase::Failed => {
                    return state.with(|s| {
                        s.error()
                            .map(super::viewer_access::failure_key)
                            .unwrap_or(Key::AccessFailed)
                    })
                }
                Phase::Ready if state.with(|s| s.command().is_some_and(Command::is_write)) => {
                    return Key::AccessReceipt
                }
                Phase::Ready | Phase::Idle => (),
            }
        }
        if current.with(Option::is_none) {
            Key::AccessUnread
        } else {
            Key::AccessIndependent
        }
    });
    let submit = Callback::new(move |_| {
        if blocked.get_untracked() || !confirmed.get_untracked() || !owner.get_untracked() {
            return;
        }
        if let Some(command) = confirmation.get_untracked() {
            dialog_open.set(false);
            on_send.run(command);
        }
    });
    view! {
      <section id="viewer-access" class="cw-access cw-block" aria-labelledby="viewer-title" hidden=move || !owner.get()>
        <div class="cw-actions"><h5 id="viewer-title">{move || text(Key::AccessTitle, locale.get())}</h5>
          <Button id="viewer-open" variant=ButtonVariant::Tonal focus_on_press=true on_press=Callback::new(move |_| open.set(!open.get_untracked()))>{move || text(if open.get() { Key::AccessClose } else { Key::AccessOpen }, locale.get())}</Button>
        </div>
        <div hidden=move || !open.get()>
          <p id="viewer-status" role="status" aria-live="polite">{move || text(status.get(), locale.get())}</p>
          <p class="cw-reason" hidden=move || !dialog_error.get()>{move || text(Key::AccessFailed, locale.get())}</p>
          <div class="cw-actions">
            <ActionButton id="viewer-read" locale blocked on_press=Callback::new(move |_| { if !blocked.get_untracked() { if let Some(work) = facts.with_untracked(|f| f.work.as_ref().map(|w| w.id.clone())) { facts.update(|f| f.viewer.current = None); on_send.run(Command::ViewerState { work }); } } })>{move || text(Key::AccessLoad, locale.get())}</ActionButton>
            <Show when=move || relevant.get() && state.with(|s| s.can_retry())><ActionButton id="viewer-retry" locale blocked=Signal::derive(move || !state.with(|s| s.can_retry())) on_press=Callback::new(move |_| on_retry.run(()))>{move || text(Key::AccessUnknown, locale.get())}</ActionButton></Show>
          </div>
          <p>{move || current.with(|s| s.as_ref().map(|s| (text(Key::AccessWorkVersion, locale.get()), " ", s.version)))}</p>
          <p>{move || text(Key::AccessAudience, locale.get())}</p>
          <div class="cw-access-audience" role="group" aria-label=move || text(Key::AccessAudience, locale.get())>
            {[ViewerAudience::Public, ViewerAudience::Restricted].map(|audience| view! {
              <ActionButton id=if audience == ViewerAudience::Public { "viewer-public" } else { "viewer-restricted" } locale variant=ButtonVariant::Tonal focus_on_press=true
                blocked=Signal::derive(move || !can_change.get() || current.with(|s| s.as_ref().is_none_or(|s| s.audience == audience)) || facts.with(|f| f.metadata.current.is_none()))
                reason=BlockReason::Metadata(Key::AccessUnread) on_press=Callback::new(move |_| {
                    let candidate = facts.with_untracked(|f| AudienceReview::new(f.viewer.current.as_ref()?, f.metadata.current.as_ref()?, audience));
                    if let (Some(candidate), Some(id)) = (candidate, operation()) {
                        let Some(request) = candidate.request(id, true) else { return; };
                        review.set(Some(candidate.clone())); prepare.run(Command::ViewerChange { work: candidate.work, request });
                    }
                })>{move || text(if audience == ViewerAudience::Public { Key::AccessPublic } else { Key::AccessRestricted }, locale.get())}</ActionButton>
            })}
            <strong role="status">{move || current.with(|s| s.as_ref().map(|s| text(if s.audience == ViewerAudience::Public { Key::AccessPublic } else { Key::AccessRestricted }, locale.get())))}</strong>
          </div>
          <p>{move || text(Key::AccessIndependent, locale.get())}</p>
          <h6>{move || text(Key::AccessPeople, locale.get())}</h6>
          <ul class="cw-access-recipients">
            <For each=move || current.with(|s| s.as_ref().map(|s| s.grants.clone()).unwrap_or_default()) key=|g| g.grant_id.clone() children=move |grant| {
                let id=grant.grant_id.clone(); let label=match grant.principal { ViewerPrincipal::Actor { actor_id } => actor_id, ViewerPrincipal::Group { group_id } => group_id };
                view! { <li><div><strong>{label}</strong><p>{move || text(Key::AccessGranted, locale.get())}</p><small>{move || expiry_label(grant.expires_at_us, locale.get())}</small></div>
                    <ActionButton id=format!("viewer-revoke-{}", id) locale blocked=Signal::derive(move || !can_change.get()) variant=ButtonVariant::Text focus_on_press=true on_press=Callback::new(move |_| change.run(ViewerAccessChange::RevokeGrant { grant_id:id.clone() }))>{move || text(Key::AccessRevoke,locale.get())}</ActionButton></li> }
            }/>
            <For each=move || current.with(|s| s.as_ref().map(|s| s.pending.clone()).unwrap_or_default()) key=|s| s.share_id.clone() children=move |share| {
                let id=share.share_id.clone();
                view! { <li><div><strong>{share.email}</strong><p>{move || text(Key::AccessPending,locale.get())}</p><small>{move || expiry_label(Some(share.expires_at_us),locale.get())}</small></div>
                  <ActionButton id=format!("viewer-revoke-share-{}",id) locale blocked=Signal::derive(move || !can_change.get()) variant=ButtonVariant::Text focus_on_press=true on_press=Callback::new(move |_| change.run(ViewerAccessChange::RevokeShare { share_id:id.clone() }))>{move || text(Key::AccessRevoke,locale.get())}</ActionButton></li> }
            }/>
          </ul>
          <div class="cw-access-form"><Field id="viewer-email" label=Key::AccessEmail value=email locale/>
            <ActionButton id="viewer-share" locale blocked=Signal::derive(move || !can_change.get() || email.with(String::is_empty)) focus_on_press=true on_press=Callback::new(move |_| { review.set(None); if let Some(id)=operation() { change.run(ViewerAccessChange::ShareMailbox { share_id:id,email:email.get_untracked() }); } })>{move || text(Key::AccessShare,locale.get())}</ActionButton>
          </div>
          <p>{move || text(Key::AccessNoMail,locale.get())}</p><p class="cw-reason">{move || text(Key::AccessHistoryMissing,locale.get())}</p>
          <details><summary>{move || text(Key::AccessActorGrant,locale.get())}</summary>
            <Field id="viewer-actor" label=Key::AccessActor value=actor locale/><Field id="viewer-expiry" label=Key::AccessExpiryInput value=until locale/>
            <p role="status" hidden=move || !invalid_expiry.get()>{move || text(Key::AccessExpiryInvalid,locale.get())}</p>
            <ActionButton id="viewer-grant-actor" locale blocked=Signal::derive(move || !can_change.get() || actor.with(String::is_empty)) focus_on_press=true on_press=Callback::new(move |_| {
                review.set(None); match expiry(&until.get_untracked()) { Ok(expires_at_us) => { invalid_expiry.set(false); if let Some(id)=operation() { change.run(ViewerAccessChange::GrantActor { grant_id:id, actor_id:actor.get_untracked(),expires_at_us }); } }, Err(())=>invalid_expiry.set(true) }
            })>{move || text(Key::AccessActorGrant,locale.get())}</ActionButton>
          </details>
          <details><summary>{move || text(Key::AccessGroups,locale.get())}</summary>
            <ActionButton id="viewer-groups-read" locale blocked on_press=Callback::new(move |_| on_send.run(Command::ViewerGroups))>{move || text(Key::AccessGroupsLoad,locale.get())}</ActionButton>
            <Field id="viewer-group-name" label=Key::AccessGroupName value=group_name locale/>
            <ActionButton id="viewer-group-create" locale blocked=Signal::derive(move || blocked.get() || group_name.with(String::is_empty)) focus_on_press=true on_press=Callback::new(move |_| {
                review.set(None); if let (Some(group),Some(operation_id))=(operation(),operation()) { prepare.run(Command::ViewerGroupChange { group,request:ViewerGroupRequest { operation_id,expected_version:0,change:ViewerGroupChange::Create { name:group_name.get_untracked() } } }); }
            })>{move || text(Key::AccessGroupCreate,locale.get())}</ActionButton>
            <For each=move || facts.with(|f| f.viewer.groups.as_ref().map(|g| g.items.clone()).unwrap_or_default()) key=|g| g.group_id.clone() children=move |group| {
              let id=group.group_id.clone(); let grant_group=id.clone(); let version=group.version;
              view! { <section class="cw-access-group"><h6>{group.name}</h6><p>{group.group_id}</p><p>{move || text(Key::AccessWorkVersion,locale.get())}" "{version}</p>
                <ul>{group.members.into_iter().map(|m| view!{<li><span>{m.actor_id}</span><small>{move ||expiry_label(m.expires_at_us,locale.get())}</small></li>}).collect_view()}</ul>
                <ActionButton id=format!("viewer-group-grant-{id}") locale blocked=Signal::derive(move || !can_change.get()) focus_on_press=true on_press=Callback::new(move |_| { review.set(None); if let Some(id)=operation() { change.run(ViewerAccessChange::GrantGroup { grant_id:id,group_id:grant_group.clone(),expires_at_us:None }); } })>{move ||text(Key::AccessGroupGrant,locale.get())}</ActionButton>

              </section> }
            }/>
            <p class="cw-reason">{move ||text(Key::AccessGroupImpactMissing,locale.get())}</p>
          </details>
          <label for="viewer-locator">{move ||text(Key::AccessLocatorLabel,locale.get())}</label><input id="viewer-locator" readonly=true prop:value=move ||facts.with(|f|f.work.as_ref().map(|w|format!("#shared/{}",w.id)).unwrap_or_default())/>
          <p>{move ||text(Key::AccessLocator,locale.get())}</p>
        </div>
        <Dialog id="viewer-confirm" title=text(Key::AccessConfirm,locale.get_untracked()) close_label=text(Key::AccessClose,locale.get_untracked()) open=Signal::derive(move ||dialog_open.get()) on_close_request=Callback::new(move |_|dialog_open.set(false)) on_error=Callback::new(move |_| {dialog_error.set(true);dialog_open.set(false);})>
          <p>{move ||facts.with(|f|f.work.as_ref().map(|w|w.details.title.clone()))}</p>
          <p>{move ||text(Key::AccessScope,locale.get())}</p>
          <div hidden=move ||review.with(Option::is_none)>
            <p>{move ||text(Key::AccessWorkVersion,locale.get())}" "{move ||review.with(|r|r.as_ref().map(|r|r.expected_version))}</p>
            <p>{move ||text(Key::MetadataVersion,locale.get())}" "{move ||review.with(|r|r.as_ref().map(|r|r.publication_version))}</p>
            <p>{move ||text(Key::MetadataHistory,locale.get())}" "{move ||review.with(|r|r.as_ref().map(|r|r.published_revision.map(super::copy::revision_label).unwrap_or_else(||text(Key::AccessNoPublication,locale.get()).into())))}</p>
          </div>
          <p class="cw-access-exact">{move || confirmation.with(|c| c.as_ref().and_then(|c| match c { Command::ViewerChange {request,..} => Some(match &request.change { ViewerAccessChange::ShareMailbox { email,.. }=>email.clone(), ViewerAccessChange::GrantActor {actor_id,..}=>actor_id.clone(),ViewerAccessChange::GrantGroup {group_id,..}=>group_id.clone(),ViewerAccessChange::RevokeGrant {grant_id}=>grant_id.clone(),ViewerAccessChange::RevokeShare {share_id}=>share_id.clone(),ViewerAccessChange::SetUnitPolicy {unit_id,..}=>unit_id.clone(),ViewerAccessChange::SetAudience {audience,..}=>text(if *audience==ViewerAudience::Public {Key::AccessPublic}else{Key::AccessRestricted},locale.get()).into()}),Command::ViewerGroupChange {group,..}=>Some(group.clone()),_=>None }))}</p>
          <p>{move || confirmation.with(|c| c.as_ref().map(|command| text(match command {
            Command::ViewerChange { request: ViewerAccessRequest { change: ViewerAccessChange::ShareMailbox { .. }, .. }, .. } => Key::AccessNoMail,
            Command::ViewerChange { request: ViewerAccessRequest { change: ViewerAccessChange::RevokeGrant { .. } | ViewerAccessChange::RevokeShare { .. } | ViewerAccessChange::SetAudience { audience: ViewerAudience::Restricted, .. }, .. }, .. } => Key::AccessRevokeHelp,
            _ => Key::AccessIndependent,
          }, locale.get())))}</p>
          <ReviewCheckbox id="viewer-confirm-checkbox" locale label=Signal::derive(move ||text(if review.with(|r|r.as_ref().is_some_and(|r|r.audience==ViewerAudience::Public)){Key::AccessConfirmRights}else{Key::AccessConfirm},locale.get()).to_owned()) checked=Signal::derive(move ||confirmed.get()) on_change=Callback::new(move |v|confirmed.set(v))/>
          <ActionButton id="viewer-confirm-apply" locale blocked=Signal::derive(move ||blocked.get() || !confirmed.get()) on_press=submit>{move ||text(Key::AccessApply,locale.get())}</ActionButton>
        </Dialog>
        <super::chapter_panel::ChapterPanel facts state locale blocked dirty=chapter_dirty on_send on_retry/>
      </section>
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn access_composition_uses_paired_copy_and_no_fake_delivery_or_global_group_removal() {
        for locale in Locale::supported() {
            Owner::new().with(|| {
                let work: CatalogWork=serde_json::from_str(include_str!("../../../../../services/cantos-server/contracts/studio/catalog/v1/fixtures/work.json")).unwrap();
                let facts=Facts {work:Some(work.clone()),..Facts::default()};
                let state=RequestState::default().context(&work.owner_id,"workspace").unwrap();
                let html=view!{<ViewerPanel facts=RwSignal::new(facts) state=RwSignal::new(state) locale=Signal::derive(move ||locale) blocked=Signal::derive(||false) dirty=RwSignal::new(false) on_send=Callback::new(|_|{}) on_retry=Callback::new(|_|{})/>}.to_html();
                assert!(html.contains(text(Key::AccessTitle,locale)) && html.contains(text(Key::AccessNoMail,locale)));
                assert!(html.contains("viewer-confirm-checkbox") && html.contains("viewer-locator"));
                assert!(html.contains(text(Key::AccessGroupImpactMissing,locale)));
                assert!(!html.contains("viewer-group-remove") && !html.contains("Invitation sent"));
            });
        }
    }
}
