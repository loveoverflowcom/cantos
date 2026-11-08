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
use cantos_ui::{Button, ButtonState, ButtonVariant, DisabledReason, LoadingState};
use leptos::prelude::*;

/// Root failures and tail failures describe one catalog request. Prefer a fresh read only
/// when its cursor is expired; a failed continuation retries the same page.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SharedRecovery {
    Restart,
    RetryPage,
}
fn shared_recovery(
    phase: PublicDiscoveryPhase,
    tail: Tail,
    has_more: bool,
) -> Option<SharedRecovery> {
    match (phase, tail) {
        (PublicDiscoveryPhase::Expired, _) | (_, Tail::Expired) => Some(SharedRecovery::Restart),
        (_, Tail::Failed(_)) if has_more => Some(SharedRecovery::RetryPage),
        (PublicDiscoveryPhase::Failed(_), _) | (_, Tail::Failed(_)) => {
            Some(SharedRecovery::Restart)
        }
        _ => None,
    }
}

#[component]
pub(crate) fn SharedLibrary(
    account: ReadSignal<AccountViewState>,
    state: RwSignal<SharedAccess>,
    locale: RwSignal<Locale>,
    on_reload: Callback<()>,
    on_more: Callback<()>,
    on_claim: Callback<()>,
) -> impl IntoView {
    // Read root signals: queued renders may outlive their route owner during cleanup.
    let admitted =
        move || state.with(|s| s.fence().is_some_and(|f| account.with(|a| a.accepts(f))));
    let recovery = move || {
        state.with(|s| shared_recovery(s.catalog.phase(), s.catalog.tail(), s.catalog.has_more()))
    };
    let has_rows = move || state.with(|s| account.with(|a| !s.visible_rows(a).is_empty()));
    let claim_state = Signal::derive(move || {
        state.with(|s| {
            if matches!(s.claim, ClaimPhase::Pending(_)) {
                DisabledReason::new(text(Key::SharedClaimPending, locale.get()))
                    .map(ButtonState::Loading)
                    .unwrap_or_default()
            } else {
                ButtonState::Enabled
            }
        })
    });
    view! {
      <Show when=admitted>
       <section id="theatre-shared-library" class="theatre-shared-library" aria-labelledby="theatre-shared-heading">
        <h2 id="theatre-shared-heading" class="theatre-section-heading" class:theatre-visually-hidden=move || !has_rows()>{move ||text(Key::SharedTitle,locale.get())}</h2>
        <Show when=move || state.with(|s| matches!(s.catalog.phase(), PublicDiscoveryPhase::Idle | PublicDiscoveryPhase::Loading) && !s.catalog.has_more())>
            <LoadingState>{move ||text(Key::SharedLoading,locale.get())}</LoadingState>
        </Show>
        <Show when=has_rows>
          <ul class="theatre-shared-rows"><For each=move ||state.with(|s|account.with(|a|s.visible_rows(a).to_vec())) key=|w|w.work_id.clone() children=move |work| view! {
            <li><a class="theatre-public-work-link" href=format!("#shared/{}",work.work_id) lang=work.language><strong>{work.title}</strong><span>{move ||text(Key::SharedBadge,locale.get())}</span></a></li>
          }/></ul>
        </Show>
        <Show when=move || recovery().is_some()>
          <div class="theatre-shared-feedback">
            {view! { <cantos_ui::Icon name=cantos_ui::IconName::WarningAmber/> }}
            <p role="status">{move ||text(Key::SharedUnavailable,locale.get())}</p>
            <Button id="theatre-shared-reload" variant=ButtonVariant::Text on_press=Callback::new(move |_| match recovery() {
                Some(SharedRecovery::Restart) => on_reload.run(()),
                Some(SharedRecovery::RetryPage) => on_more.run(()),
                None => (),
            })>{move ||text(Key::PublicRetry,locale.get())}</Button>
          </div>
        </Show>
        <Show when=move || state.with(|s| s.catalog.tail() == Tail::More && s.catalog.has_more())>
          <Button id="theatre-shared-more" variant=ButtonVariant::Text on_press=Callback::new(move |_|on_more.run(()))>{move ||text(Key::DiscoveryLoadMore,locale.get())}</Button>
        </Show>
        <Show when=move || state.with(|s| s.catalog.tail() == Tail::Loading && s.catalog.has_more())>
          <LoadingState>{move ||text(Key::SharedLoading,locale.get())}</LoadingState>
        </Show>
        <details class="theatre-shared-disclosure">
          <summary>{move ||text(Key::SharedTitle,locale.get())}</summary>
          <p class="theatre-muted">{move ||text(Key::SharedClaimHelp,locale.get())}</p>
          <Button id="theatre-shared-claim" variant=ButtonVariant::Text state=claim_state focus_on_press=true on_press=Callback::new(move |_|on_claim.run(()))>{move ||text(Key::SharedClaim,locale.get())}</Button>
        </details>
        <p id="theatre-shared-claim-status" role="status" hidden=move ||state.with(|s|matches!(s.claim,ClaimPhase::Idle))>{move ||state.with(|s|match &s.claim { ClaimPhase::Idle=>None,ClaimPhase::Pending(_)=>Some(text(Key::SharedClaimPending,locale.get())),ClaimPhase::Unknown(_)=>Some(text(Key::SharedClaimUnknown,locale.get())),ClaimPhase::Done(_)=>Some(text(Key::SharedClaimDone,locale.get())),ClaimPhase::Failed=>Some(text(Key::SharedClaimFailed,locale.get())) })}</p>
       </section>
      </Show>
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
    pub(super) fn signed_in() -> AccountViewState {
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

#[cfg(test)]
mod recovery_tests {
    use super::*;
    use crate::application::public_discovery::{
        DiscoveryQuery, DiscoveryReadError, PublicDiscoveryFailure,
    };
    #[test]
    fn root_and_tail_failure_have_one_message_and_one_correct_recovery() {
        assert_eq!(
            shared_recovery(
                PublicDiscoveryPhase::Failed(PublicDiscoveryFailure::Unavailable),
                Tail::Failed(PublicDiscoveryFailure::Unavailable),
                false
            ),
            Some(SharedRecovery::Restart)
        );
        assert_eq!(
            shared_recovery(
                PublicDiscoveryPhase::Ready,
                Tail::Failed(PublicDiscoveryFailure::Protocol),
                true
            ),
            Some(SharedRecovery::RetryPage)
        );
        assert_eq!(
            shared_recovery(PublicDiscoveryPhase::Ready, Tail::Expired, true),
            Some(SharedRecovery::Restart)
        );
        assert_eq!(
            shared_recovery(
                PublicDiscoveryPhase::Failed(PublicDiscoveryFailure::Unavailable),
                Tail::Failed(PublicDiscoveryFailure::Unavailable),
                true
            ),
            Some(SharedRecovery::RetryPage)
        );
        for phase in [
            PublicDiscoveryPhase::Idle,
            PublicDiscoveryPhase::Loading,
            PublicDiscoveryPhase::Empty,
            PublicDiscoveryPhase::Ready,
            PublicDiscoveryPhase::Incomplete,
        ] {
            assert_eq!(shared_recovery(phase, Tail::End, false), None);
        }
        for tail_failure in [false, true] {
            for locale in Locale::supported() {
                Owner::new().with(|| {
                    let account = super::tests::signed_in();
                    let mut shared = SharedAccess::default();
                    shared.observe(account.fence());
                    let first = shared.catalog.begin(DiscoveryQuery::default()).unwrap();
                    let request = if tail_failure {
                        let mut page: crate::contract::discovery::MetadataWorkPage = serde_json::from_str(include_str!("../../../../../services/cantos-server/contracts/theatre/discovery/v1/fixtures/metadata/webp_works.json")).unwrap();
                        page.next_cursor=Some("page-two".into());
                        shared.catalog.finish(&first, Ok(page));
                        shared.catalog.load_more().unwrap()
                    } else {first};
                    shared.catalog.finish(&request, Err(DiscoveryReadError::Unavailable));
                    let html=view! { <SharedLibrary account=RwSignal::new(account).read_only() state=RwSignal::new(shared) locale=RwSignal::new(locale) on_reload=Callback::new(|_|{}) on_more=Callback::new(|_|{}) on_claim=Callback::new(|_|{})/> }.to_html();
                    assert_eq!(html.matches(text(Key::SharedUnavailable,locale)).count(),1, "{html}");
                    assert_eq!(html.matches("id=\"theatre-shared-reload\"").count(),1);
                    assert!(!html.contains("id=\"theatre-shared-more\""));
                    assert!(!html.contains("theatre-panel theatre-shared-library"));
                    assert!(html.contains("<details class=\"theatre-shared-disclosure\"><summary>"));
                    assert!(!html.contains("<details open"));
                    // A failed private read still obeys visible_rows; no retained private title leak.
                    assert!(!html.contains("href=\"#shared/"));
                });
            }
        }
    }
}
