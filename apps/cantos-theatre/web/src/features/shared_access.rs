//! Shared Library and generic deep-link state. No public fallback or private restore controls.
use crate::{
    application::{
        account_view::{AccountViewState, SessionPhase},
        public_discovery::{PublicDiscoveryPhase, Tail},
        public_work::{WorkDetailPhase, WorkDetailState},
        shared_access::{ClaimPhase, SharedAccess},
    },
    features::public_work::{ListenControls, PublicWorkDetail},
    messages::{text, Key, Locale},
};
use cantos_ui::{Button, ButtonVariant, LoadingState};
use leptos::prelude::*;

#[component]
pub(crate) fn SharedLibrary(
    account: ReadSignal<AccountViewState>,
    state: RwSignal<SharedAccess>,
    locale: RwSignal<Locale>,
    on_reload: Callback<()>,
    on_more: Callback<()>,
    on_claim: Callback<()>,
) -> impl IntoView {
    // Session loss and route exit notify these readers while the Library owner is cleaned; the
    // view reads the root signals directly instead of a disposed arena projection.
    let admitted =
        move || state.with(|s| s.fence().is_some_and(|f| account.with(|a| a.accepts(f))));
    view! {
      <Show when=admitted><section id="theatre-shared-library" class="theatre-panel theatre-shared-library" aria-labelledby="theatre-shared-heading">
        <h2 id="theatre-shared-heading">{move ||text(Key::SharedTitle,locale.get())}</h2>
        {move || match state.with(|s|s.catalog.phase()) {
            PublicDiscoveryPhase::Idle | PublicDiscoveryPhase::Loading => view!{<LoadingState>{move ||text(Key::SharedLoading,locale.get())}</LoadingState>}.into_any(),
            PublicDiscoveryPhase::Empty => view!{<p role="status">{move ||text(Key::SharedEmpty,locale.get())}</p>}.into_any(),
            PublicDiscoveryPhase::Expired | PublicDiscoveryPhase::Failed(_) => view!{<p role="status">{move ||text(Key::SharedUnavailable,locale.get())}</p>}.into_any(),
            PublicDiscoveryPhase::Ready | PublicDiscoveryPhase::Incomplete => ().into_any(),
        }}
        <ul class="theatre-shared-rows"><For each=move ||state.with(|s|account.with(|a|s.visible_rows(a).to_vec())) key=|w|w.work_id.clone() children=move |work| view! {
          <li><a class="theatre-public-work-link" href=format!("#shared/{}",work.work_id) lang=work.language><strong>{work.title}</strong><span>{move ||text(Key::SharedBadge,locale.get())}</span></a></li>
        }/></ul>
        <p role="status">{move || matches!(state.with(|s|s.catalog.tail()),Tail::Failed(_)).then(||text(Key::SharedUnavailable,locale.get()))}</p>
        <div class="theatre-actions">
          <Button id="theatre-shared-reload" variant=ButtonVariant::Text on_press=Callback::new(move |_|on_reload.run(()))>{move ||text(Key::SharedRefresh,locale.get())}</Button>
          <Show when=move ||state.with(|s|s.catalog.has_more())><Button id="theatre-shared-more" variant=ButtonVariant::Tonal on_press=Callback::new(move |_|on_more.run(()))>{move ||text(Key::DiscoveryLoadMore,locale.get())}</Button></Show>
          <Button id="theatre-shared-claim" variant=ButtonVariant::Tonal focus_on_press=true on_press=Callback::new(move |_|on_claim.run(()))>{move ||text(Key::SharedClaim,locale.get())}</Button>
        </div>
        <p>{move ||text(Key::SharedClaimHelp,locale.get())}</p>
        <p id="theatre-shared-claim-status" role="status">{move ||state.with(|s|match &s.claim { ClaimPhase::Idle=>None,ClaimPhase::Pending(_)=>Some(text(Key::SharedClaimPending,locale.get())),ClaimPhase::Unknown(_)=>Some(text(Key::SharedClaimUnknown,locale.get())),ClaimPhase::Done(_)=>Some(text(Key::SharedClaimDone,locale.get())),ClaimPhase::Failed=>Some(text(Key::SharedClaimFailed,locale.get())) })}</p>
      </section></Show>
    }
}

#[component]
pub(crate) fn SharedDetail(
    work: crate::core::public_ids::PublicWorkId,
    account: ReadSignal<AccountViewState>,
    state: RwSignal<WorkDetailState>,
    scope: RwSignal<Option<crate::application::account::RequestFence>>,
    locale: RwSignal<Locale>,
    listen: ListenControls,
    on_login: Callback<()>,
    on_back: Callback<()>,
    on_retry: Callback<()>,
    on_more: Callback<()>,
) -> impl IntoView {
    // As in SharedLibrary, the admission check reads root signals, never a disposed projection.
    let admitted = move || {
        scope.with(|f| f.as_ref().is_some_and(|f| account.with(|a| a.accepts(f))))
            && state.with(|s| s.work_id() == Some(work))
    };
    view! {
      <Show when=admitted fallback=move ||view!{
        <section class="theatre-public-state theatre-panel" aria-labelledby="theatre-shared-generic-title">
          <h1 id="theatre-shared-generic-title" tabindex="-1">{move ||text(Key::SharedGeneric,locale.get())}</h1>
          <p>{move ||text(Key::SharedDenied,locale.get())}</p>
          <a class="theatre-action theatre-action-tonal" href="#library">{move ||text(Key::SharedBack,locale.get())}</a>
          <Show when=move ||account.with(|s|s.phase()==SessionPhase::Guest)><Button id="theatre-shared-login" on_press=Callback::new(move |_|on_login.run(()))>{move ||text(Key::SharedLogin,locale.get())}</Button></Show>
        </section>
      }>
        <Show when=move ||state.with(|s|matches!(s.phase(),WorkDetailPhase::Ready{..}))>
          <p class="theatre-shared-badge">{move ||text(Key::SharedBadge,locale.get())}</p>
          <p class="theatre-muted">{move ||text(Key::SharedRestoreUnavailable,locale.get())}</p>
        </Show>
        <PublicWorkDetail state locale listen on_back on_retry on_load_more=on_more shared=true/>
      </Show>
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn shared_library_is_localized_and_pending_claim_never_announces_invitation_delivery() {
        for locale in Locale::supported() {
            Owner::new().with(||{
                let mut account=AccountViewState::default(); let issued=account.begin_refresh().unwrap();
                account.finish_session(issued,Ok(crate::application::account::AccountScope::from_session("viewer".into(),uuid::Uuid::from_u128(1).to_string()).unwrap()),||uuid::Uuid::from_u128(3).to_string()).unwrap();
                let mut state=SharedAccess::default(); state.observe(account.fence()); state.claim=ClaimPhase::Unknown(uuid::Uuid::from_u128(1).to_string());
                let html=view!{<SharedLibrary account=RwSignal::new(account).read_only() state=RwSignal::new(state) locale=RwSignal::new(locale) on_reload=Callback::new(|_|{}) on_more=Callback::new(|_|{}) on_claim=Callback::new(|_|{})/>}.to_html();
                assert!(html.contains(text(Key::SharedTitle,locale)));
                assert!(html.contains(text(Key::SharedClaimUnknown,locale)));
                assert!(!html.contains("Invitation sent") && !html.contains("Đã gửi"));
                assert!(!html.contains("#work/") && !html.contains("media/"));
            });
        }
    }
    fn signed_in() -> AccountViewState {
        let mut account = AccountViewState::default();
        let issued = account.begin_refresh().unwrap();
        account
            .finish_session(
                issued,
                Ok(crate::application::account::AccountScope::from_session(
                    "viewer".into(),
                    uuid::Uuid::from_u128(1).to_string(),
                )
                .unwrap()),
                || uuid::Uuid::from_u128(3).to_string(),
            )
            .unwrap();
        account
    }
    fn sign_out(account: &mut AccountViewState) {
        let issued = account.begin_refresh().unwrap();
        let _ = account.finish_session(
            issued,
            Err(crate::application::account_view::AccountFailure::SignedOut),
            || uuid::Uuid::from_u128(4).to_string(),
        );
    }
    /// Session loss and route exit clean the Shared owners while queued renders still hold their
    /// views; each must read root state, not a disposed projection of its owner.
    #[test]
    fn shared_library_and_detail_retain_admission_through_owner_cleanup() {
        Owner::new().with(|| {
            let account = RwSignal::new(signed_in());
            let mut access = SharedAccess::default();
            access.observe(account.with_untracked(AccountViewState::fence));
            // Root-owned inputs, as the live shell supplies them.
            let (shared, locale) = (RwSignal::new(access), RwSignal::new(Locale::Vietnamese));
            let (reader, ignore) = (account.read_only(), Callback::new(|_: ()| {}));
            let library_owner = Owner::new();
            let library = library_owner.with(|| view! {
                <SharedLibrary account=reader state=shared locale on_reload=ignore on_more=ignore on_claim=ignore/>
            });
            let work = crate::core::public_ids::PublicWorkId::parse("10000000-0000-4000-8000-000000000001").unwrap();
            let detail_state = RwSignal::new(WorkDetailState::default());
            let scope = RwSignal::new(account.with_untracked(AccountViewState::fence));
            let listen = ListenControls {
                player: crate::application::player_session::PlayerSession::new(crate::infrastructure::inert_media::InertMedia),
                listen: RwSignal::new(crate::application::public_listen::ListenState::default()),
                on_listen: Callback::new(|_| ()),
            };
            let detail_owner = Owner::new();
            let detail = detail_owner.with(|| view! {
                <SharedDetail work account=reader state=detail_state scope locale listen
                    on_login=ignore on_back=ignore on_retry=ignore on_more=ignore/>
            });
            library_owner.cleanup();
            detail_owner.cleanup();
            account.update(sign_out);
            let library = library.to_html();
            assert!(!library.contains("theatre-shared-library"), "{library}");
            let detail = detail.to_html();
            assert!(detail.contains(text(Key::SharedDenied, Locale::Vietnamese)), "{detail}");
        });
    }
}
