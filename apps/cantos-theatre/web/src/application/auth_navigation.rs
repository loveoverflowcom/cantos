//! Internal auth return routes and request fencing, independent of browser history and identity IO.
//!
//! A route is presentation intent, never proof of authentication or registration. The shell must
//! confirm the current session with the API before treating the listener as signed in.

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ReturnDestination {
    SharedWork(crate::core::public_ids::PublicWorkId),
    #[default]
    Home,
    Account,
    Library,
    Membership,
    MembershipFromAccount,
}

impl ReturnDestination {
    pub fn slug(self) -> String {
        match self {
            Self::SharedWork(id) => format!("shared/{id}"),
            Self::Home => "home".into(),
            Self::Account => "account".into(),
            Self::Library => "library".into(),
            Self::Membership => "membership".into(),
            Self::MembershipFromAccount => "membership-account".into(),
        }
    }

    fn from_slug(value: &str) -> Option<Self> {
        if let Some(id) = value.strip_prefix("shared/") {
            return crate::core::public_ids::PublicWorkId::parse(id).map(Self::SharedWork);
        }
        match value {
            "home" => Some(Self::Home),
            "account" => Some(Self::Account),
            "library" => Some(Self::Library),
            "membership" => Some(Self::Membership),
            "membership-account" => Some(Self::MembershipFromAccount),
            _ => None,
        }
    }

    pub fn target(self) -> NavigationTarget {
        match self {
            Self::SharedWork(id) => NavigationTarget::SharedWork(id),
            Self::Home => NavigationTarget::Home,
            Self::Account => NavigationTarget::Account,
            Self::Library => NavigationTarget::Library,
            Self::Membership => NavigationTarget::Membership(MembershipReturn::Home),
            Self::MembershipFromAccount => NavigationTarget::Membership(MembershipReturn::Account),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AuthStep {
    Login,
    Register,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AuthRoute {
    pub step: AuthStep,
    pub origin: ReturnDestination,
}

impl AuthRoute {
    pub fn login(origin: ReturnDestination) -> Self {
        Self {
            step: AuthStep::Login,
            origin,
        }
    }

    pub fn register(origin: ReturnDestination) -> Self {
        Self {
            step: AuthStep::Register,
            origin,
        }
    }

    /// Only internal named destinations are accepted; absent or malformed context falls home.
    pub fn from_hash(hash: &str) -> Option<Self> {
        let (page, context) = split_hash(hash);
        let step = match page {
            "login" => AuthStep::Login,
            "register" => AuthStep::Register,
            _ => return None,
        };
        let origin = context
            .and_then(|value| value.strip_prefix("from="))
            .and_then(ReturnDestination::from_slug)
            .unwrap_or_default();
        Some(Self { step, origin })
    }

    pub fn hash(self) -> String {
        let page = match self.step {
            AuthStep::Login => "login",
            AuthStep::Register => "register",
        };
        format!("#{page}?from={}", self.origin.slug())
    }

    pub fn back(self) -> NavigationTarget {
        match self.step {
            AuthStep::Login => self.origin.target(),
            AuthStep::Register => NavigationTarget::Auth(Self::login(self.origin)),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum MembershipReturn {
    #[default]
    Home,
    Account,
}

impl MembershipReturn {
    pub fn from_hash(hash: &str) -> Option<Self> {
        let (page, context) = split_hash(hash);
        if page != "membership" {
            return None;
        }
        let target = match context.and_then(|value| value.strip_prefix("from=")) {
            Some("account") => Self::Account,
            _ => Self::Home,
        };
        Some(target)
    }

    pub fn hash(self) -> String {
        let source = match self {
            Self::Home => "home",
            Self::Account => "account",
        };
        format!("#membership?from={source}")
    }

    pub fn back(self) -> NavigationTarget {
        match self {
            Self::Home => NavigationTarget::Home,
            Self::Account => NavigationTarget::Account,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NavigationTarget {
    SharedWork(crate::core::public_ids::PublicWorkId),
    Home,
    Account,
    Library,
    Auth(AuthRoute),
    Membership(MembershipReturn),
}

fn split_hash(hash: &str) -> (&str, Option<&str>) {
    let value = hash.strip_prefix('#').unwrap_or(hash);
    match value.split_once('?') {
        Some((page, context)) => (page, Some(context)),
        None => (value, None),
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AuthIntentError {
    Closed,
    RequestPending,
    EpochExhausted,
}

/// The captured route and epoch are nonsecret and have no persistence or transport representation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AuthRequestFence {
    epoch: u64,
    route: AuthRoute,
}

impl AuthRequestFence {
    pub fn route(self) -> AuthRoute {
        self.route
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AuthIntentState {
    current: Option<AuthRoute>,
    epoch: u64,
    pending: Option<AuthRequestFence>,
    exhausted: bool,
}

impl AuthIntentState {
    pub fn new(current: Option<AuthRoute>) -> Self {
        Self {
            current,
            ..Self::default()
        }
    }

    /// Observe hash/history navigation without invalidating a still-current render observation.
    pub fn observe_route(&mut self, route: Option<AuthRoute>) -> Result<(), AuthIntentError> {
        if self.current != route {
            self.current = route;
            self.invalidate()?;
        }
        Ok(())
    }

    /// Back, login cancel and logout call this before starting any asynchronous side effect.
    pub fn invalidate(&mut self) -> Result<(), AuthIntentError> {
        self.pending = None;
        self.epoch = self.epoch.checked_add(1).ok_or_else(|| {
            self.exhausted = true;
            AuthIntentError::EpochExhausted
        })?;
        Ok(())
    }

    pub fn begin(&mut self) -> Result<AuthRequestFence, AuthIntentError> {
        if self.exhausted {
            return Err(AuthIntentError::EpochExhausted);
        }
        let route = self.current.ok_or(AuthIntentError::Closed)?;
        if self.pending.is_some() {
            return Err(AuthIntentError::RequestPending);
        }
        self.invalidate()?;
        let issued = AuthRequestFence {
            epoch: self.epoch,
            route,
        };
        self.pending = Some(issued);
        Ok(issued)
    }

    pub fn accepts(&self, issued: AuthRequestFence) -> bool {
        !self.exhausted
            && self.current == Some(issued.route)
            && self.epoch == issued.epoch
            && self.pending == Some(issued)
    }

    /// Consumes the pending intent once; the shell still owes current-session confirmation.
    pub fn finish(&mut self, issued: AuthRequestFence) -> bool {
        if !self.accepts(issued) {
            return false;
        }
        self.pending = None;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn library_auth_context_roundtrips_without_accepting_redirect_parameters() {
        for page in ["login", "register"] {
            let hash = format!("#{page}?from=library");
            let route = AuthRoute::from_hash(&hash).unwrap();
            assert_eq!(route.origin.slug(), "library");
            assert_eq!(route.hash(), hash);
        }
        for context in [
            "from=library&redirect=https://example.com",
            "from=library&from=account",
            "from=%6cibrary",
            "from=library#account",
            "from=/library",
        ] {
            assert_eq!(
                AuthRoute::from_hash(&format!("#login?{context}")),
                Some(AuthRoute::login(ReturnDestination::Home))
            );
        }
    }

    const ORIGINS: [ReturnDestination; 5] = [
        ReturnDestination::Home,
        ReturnDestination::Account,
        ReturnDestination::Library,
        ReturnDestination::Membership,
        ReturnDestination::MembershipFromAccount,
    ];

    #[test]
    fn login_and_registration_back_preserve_each_internal_origin() {
        let cases = [
            (ReturnDestination::Home, NavigationTarget::Home),
            (ReturnDestination::Account, NavigationTarget::Account),
            (ReturnDestination::Library, NavigationTarget::Library),
            (
                ReturnDestination::Membership,
                NavigationTarget::Membership(MembershipReturn::Home),
            ),
            (
                ReturnDestination::MembershipFromAccount,
                NavigationTarget::Membership(MembershipReturn::Account),
            ),
        ];
        for (origin, expected) in cases {
            let login = AuthRoute::login(origin);
            assert_eq!(login.back(), expected);
            assert_eq!(
                AuthRoute::register(origin).back(),
                NavigationTarget::Auth(login)
            );
        }
    }

    #[test]
    fn hash_reconstruction_restores_back_targets_without_a_browser_stack() {
        let cases = [
            ("#login?from=account", NavigationTarget::Account),
            ("#login?from=library", NavigationTarget::Library),
            (
                "#register?from=library",
                NavigationTarget::Auth(AuthRoute::login(ReturnDestination::Library)),
            ),
            (
                "#register?from=membership",
                NavigationTarget::Auth(AuthRoute::login(ReturnDestination::Membership)),
            ),
            ("#login", NavigationTarget::Home),
            (
                "#login?from=membership-account",
                NavigationTarget::Membership(MembershipReturn::Account),
            ),
            (
                "#register",
                NavigationTarget::Auth(AuthRoute::login(ReturnDestination::Home)),
            ),
        ];
        for (hash, expected) in cases {
            assert_eq!(AuthRoute::from_hash(hash).unwrap().back(), expected);
        }
        for origin in ORIGINS {
            for route in [AuthRoute::login(origin), AuthRoute::register(origin)] {
                assert_eq!(AuthRoute::from_hash(&route.hash()), Some(route));
            }
        }
    }

    #[test]
    fn caller_urls_unknown_queries_and_encoded_redirects_never_become_return_routes() {
        for context in [
            "from=https://example.com",
            "from=//example.com",
            "from=%2F%2Fexample.com",
            "return_to=account",
            "from=account&redirect=https://example.com",
            "from=account&from=membership",
            "from=studio",
            "from=account#https://example.com",
        ] {
            assert_eq!(
                AuthRoute::from_hash(&format!("#login?{context}")),
                Some(AuthRoute::login(ReturnDestination::Home))
            );
            assert_eq!(
                MembershipReturn::from_hash(&format!("#membership?{context}")),
                Some(MembershipReturn::Home)
            );
        }
        for hash in ["#unknown", "https://example.com/#login", "#studio/login"] {
            assert_eq!(AuthRoute::from_hash(hash), None);
            assert_eq!(MembershipReturn::from_hash(hash), None);
        }
    }

    #[test]
    fn membership_back_names_only_internal_destinations() {
        assert_eq!(MembershipReturn::Account.back(), NavigationTarget::Account);
        assert_eq!(MembershipReturn::Home.back(), NavigationTarget::Home);
        for member in [MembershipReturn::Home, MembershipReturn::Account] {
            assert_eq!(MembershipReturn::from_hash(&member.hash()), Some(member));
        }
        // The retired provider completion contexts are ordinary unknown contexts now.
        for context in ["registered-home", "registered-account"] {
            assert_eq!(
                MembershipReturn::from_hash(&format!("#membership?from={context}")),
                Some(MembershipReturn::Home)
            );
        }
        assert_eq!(
            AuthRoute::from_hash("#registration-complete?from=home"),
            None
        );
    }

    #[test]
    fn repeated_observation_and_duplicate_submit_keep_one_pending_request() {
        let route = AuthRoute::login(ReturnDestination::Account);
        let mut state = AuthIntentState::new(Some(route));
        let issued = state.begin().unwrap();
        assert_eq!(state.observe_route(Some(route)), Ok(()));
        assert!(state.accepts(issued));
        assert_eq!(state.begin(), Err(AuthIntentError::RequestPending));
        assert!(state.finish(issued));
        assert!(!state.finish(issued));
    }

    #[test]
    fn back_and_forward_cannot_revive_the_same_route_old_request() {
        let route = AuthRoute::login(ReturnDestination::Home);
        let mut state = AuthIntentState::new(Some(route));
        let old = state.begin().unwrap();
        assert_eq!(state.observe_route(None), Ok(()));
        assert!(!state.finish(old));
        assert_eq!(state.begin(), Err(AuthIntentError::Closed));
        assert_eq!(state.observe_route(Some(route)), Ok(()));
        let current = state.begin().unwrap();
        assert!(!state.finish(old));
        assert!(state.accepts(current));
        assert!(state.finish(current));
    }

    #[test]
    fn cancel_logout_and_route_switch_reject_late_success_and_failure_completions() {
        let login = AuthRoute::login(ReturnDestination::Account);
        for destination in [
            None,
            Some(AuthRoute::register(ReturnDestination::Account)),
            Some(AuthRoute::login(ReturnDestination::Membership)),
        ] {
            let mut state = AuthIntentState::new(Some(login));
            let old = state.begin().unwrap();
            assert_eq!(state.observe_route(destination), Ok(()));
            assert!(!state.accepts(old));
            assert!(!state.finish(old));
        }
        for _intent in ["cancel", "logout", "back-before-hashchange"] {
            let mut state = AuthIntentState::new(Some(login));
            let old = state.begin().unwrap();
            assert_eq!(state.invalidate(), Ok(()));
            let current = state.begin().unwrap();
            assert!(!state.finish(old));
            assert!(state.finish(current));
            assert_eq!(current.route(), login);
        }
    }

    #[test]
    fn epoch_exhaustion_drops_pending_work_and_permanently_fails_closed() {
        let route = AuthRoute::login(ReturnDestination::Home);
        let mut state = AuthIntentState {
            current: Some(route),
            epoch: u64::MAX - 1,
            ..AuthIntentState::default()
        };
        let old = state.begin().unwrap();
        assert!(state.accepts(old));
        assert_eq!(state.invalidate(), Err(AuthIntentError::EpochExhausted));
        assert!(!state.finish(old));
        assert_eq!(
            state.observe_route(None),
            Err(AuthIntentError::EpochExhausted)
        );
        assert_eq!(
            state.observe_route(Some(route)),
            Err(AuthIntentError::EpochExhausted)
        );
        assert_eq!(state.begin(), Err(AuthIntentError::EpochExhausted));
    }
}
