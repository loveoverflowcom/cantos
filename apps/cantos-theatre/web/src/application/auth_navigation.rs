//! Internal auth return routes and request fencing, independent of browser history and identity IO.
//!
//! A route is presentation intent, never proof of authentication or registration. The shell must
//! confirm the current session and a supported provider registration flow before showing completion.

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ReturnDestination {
    #[default]
    Home,
    Account,
    Membership,
    MembershipFromAccount,
}

impl ReturnDestination {
    pub fn slug(self) -> &'static str {
        match self {
            Self::Home => "home",
            Self::Account => "account",
            Self::Membership => "membership",
            Self::MembershipFromAccount => "membership-account",
        }
    }

    fn from_slug(value: &str) -> Option<Self> {
        match value {
            "home" => Some(Self::Home),
            "account" => Some(Self::Account),
            "membership" => Some(Self::Membership),
            "membership-account" => Some(Self::MembershipFromAccount),
            _ => None,
        }
    }

    pub fn target(self) -> NavigationTarget {
        match self {
            Self::Home => NavigationTarget::Home,
            Self::Account => NavigationTarget::Account,
            Self::Membership => NavigationTarget::Membership(MembershipReturn::Home),
            Self::MembershipFromAccount => NavigationTarget::Membership(MembershipReturn::Account),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AuthStep {
    Login,
    Register,
    RegistrationComplete,
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

    pub fn registration_complete(origin: ReturnDestination) -> Self {
        Self {
            step: AuthStep::RegistrationComplete,
            origin,
        }
    }

    /// Only internal named destinations are accepted; absent or malformed context falls home.
    pub fn from_hash(hash: &str) -> Option<Self> {
        let (page, context) = split_hash(hash);
        let step = match page {
            "login" => AuthStep::Login,
            "register" => AuthStep::Register,
            "registration-complete" => AuthStep::RegistrationComplete,
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
            AuthStep::RegistrationComplete => "registration-complete",
        };
        format!("#{page}?from={}", self.origin.slug())
    }

    pub fn back(self) -> NavigationTarget {
        match self.step {
            AuthStep::Login | AuthStep::RegistrationComplete => self.origin.target(),
            AuthStep::Register => NavigationTarget::Auth(Self::login(self.origin)),
        }
    }

    pub fn registration_plans(self) -> NavigationTarget {
        NavigationTarget::Membership(MembershipReturn::RegistrationComplete(self.origin))
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum MembershipReturn {
    #[default]
    Home,
    Account,
    RegistrationComplete(ReturnDestination),
}

impl MembershipReturn {
    pub fn from_hash(hash: &str) -> Option<Self> {
        let (page, context) = split_hash(hash);
        if page != "membership" {
            return None;
        }
        let target = match context.and_then(|value| value.strip_prefix("from=")) {
            Some("account") => Self::Account,
            Some("registered-home") => Self::RegistrationComplete(ReturnDestination::Home),
            Some("registered-account") => Self::RegistrationComplete(ReturnDestination::Account),
            Some("registered-membership") => {
                Self::RegistrationComplete(ReturnDestination::Membership)
            }
            Some("registered-membership-account") => {
                Self::RegistrationComplete(ReturnDestination::MembershipFromAccount)
            }
            _ => Self::Home,
        };
        Some(target)
    }

    pub fn hash(self) -> String {
        let source = match self {
            Self::Home => "home",
            Self::Account => "account",
            Self::RegistrationComplete(ReturnDestination::Home) => "registered-home",
            Self::RegistrationComplete(ReturnDestination::Account) => "registered-account",
            Self::RegistrationComplete(ReturnDestination::Membership) => "registered-membership",
            Self::RegistrationComplete(ReturnDestination::MembershipFromAccount) => {
                "registered-membership-account"
            }
        };
        format!("#membership?from={source}")
    }

    pub fn back(self) -> NavigationTarget {
        match self {
            Self::Home => NavigationTarget::Home,
            Self::Account => NavigationTarget::Account,
            Self::RegistrationComplete(origin) => {
                NavigationTarget::Auth(AuthRoute::registration_complete(origin))
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NavigationTarget {
    Home,
    Account,
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
pub enum ProviderReturnResponse {
    Completed,
    Cancelled,
}

/// Nonsecret UI correlation only. A matching response still proves no current session.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProviderReturn {
    route: AuthRoute,
    intent: String,
}

impl ProviderReturn {
    pub fn new(route: AuthRoute, intent: impl Into<String>) -> Option<Self> {
        let intent = intent.into();
        (route.step == AuthStep::Login && valid_provider_intent(&intent))
            .then_some(Self { route, intent })
    }

    pub fn route(&self) -> AuthRoute {
        self.route
    }

    pub fn intent(&self) -> &str {
        &self.intent
    }

    pub fn serialize(&self) -> String {
        format!("{}|{}", self.intent, self.route.hash())
    }

    /// Stored text must be the exact canonical encoding, including its closed return context.
    pub fn parse(stored: &str) -> Option<Self> {
        let (intent, hash) = stored.split_once('|')?;
        let route = AuthRoute::from_hash(hash)?;
        let value = Self::new(route, intent)?;
        (value.serialize() == stored).then_some(value)
    }

    /// A mismatched old callback is inert. The shell must retain newer stored context B when
    /// callback A returns `None`; only `Some` permits consuming this exact context.
    pub fn response(&self, hash: &str) -> Option<ProviderReturnResponse> {
        let (response, intent) = parse_provider_callback(hash)?;
        (intent == self.intent).then_some(response)
    }
}

pub fn valid_provider_intent(intent: &str) -> bool {
    intent.len() == 32
        && intent
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

/// Only the fixed callback hashes and one exact correlation field are accepted. No route,
/// arbitrary URL, provider error text or registration completion comes from this callback.
pub fn parse_provider_callback(hash: &str) -> Option<(ProviderReturnResponse, &str)> {
    let (response, intent) = if let Some(intent) = hash.strip_prefix("#auth-return?intent=") {
        (ProviderReturnResponse::Completed, intent)
    } else {
        (
            ProviderReturnResponse::Cancelled,
            hash.strip_prefix("#auth-cancelled?intent=")?,
        )
    };
    valid_provider_intent(intent).then_some((response, intent))
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

    /// Back, provider cancel and logout call this before starting any asynchronous side effect.
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
        if route.step == AuthStep::RegistrationComplete {
            return Err(AuthIntentError::Closed);
        }
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

    const ORIGINS: [ReturnDestination; 4] = [
        ReturnDestination::Home,
        ReturnDestination::Account,
        ReturnDestination::Membership,
        ReturnDestination::MembershipFromAccount,
    ];

    #[test]
    fn login_and_registration_back_preserve_each_internal_origin() {
        let cases = [
            (ReturnDestination::Home, NavigationTarget::Home),
            (ReturnDestination::Account, NavigationTarget::Account),
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
            for route in [
                AuthRoute::login(origin),
                AuthRoute::register(origin),
                AuthRoute::registration_complete(origin),
            ] {
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
    fn membership_opened_from_registration_returns_to_the_same_completion_intent() {
        for origin in ORIGINS {
            let complete = AuthRoute::registration_complete(origin);
            let NavigationTarget::Membership(membership) = complete.registration_plans() else {
                panic!("registration plans must stay inside the app");
            };
            assert_eq!(
                MembershipReturn::from_hash(&membership.hash()),
                Some(membership)
            );
            assert_eq!(membership.back(), NavigationTarget::Auth(complete));
        }
        assert_eq!(MembershipReturn::Account.back(), NavigationTarget::Account);
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
        for _intent in ["provider-cancel", "logout", "back-before-hashchange"] {
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
    fn direct_completion_route_cannot_start_a_provider_request_or_confirm_registration() {
        let route = AuthRoute::from_hash("#registration-complete?from=account").unwrap();
        let mut state = AuthIntentState::new(Some(route));
        assert_eq!(state.begin(), Err(AuthIntentError::Closed));
        // Parsing creates only a destination value; no identity/session/capability is present.
        assert_eq!(route.origin, ReturnDestination::Account);
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

    #[test]
    fn provider_context_round_trips_only_canonical_login_routes_and_lowercase_hex_intents() {
        let intent = "0123456789abcdef0123456789abcdef";
        for origin in ORIGINS {
            let route = AuthRoute::login(origin);
            let context = ProviderReturn::new(route, intent).unwrap();
            assert_eq!(context.route(), route);
            assert_eq!(context.intent(), intent);
            assert_eq!(ProviderReturn::parse(&context.serialize()), Some(context));
        }
        for invalid in [
            "",
            "0123456789abcdef0123456789abcde",
            "0123456789abcdef0123456789abcdef0",
            "0123456789ABCDEF0123456789ABCDEF",
            "0123456789abcdef0123456789abcdeg",
        ] {
            assert!(!valid_provider_intent(invalid));
            assert!(
                ProviderReturn::new(AuthRoute::login(ReturnDestination::Home), invalid).is_none()
            );
        }
        for hash in [
            "#login",
            "#login?from=https://example.com",
            "#login?from=account&from=home",
            "#login?from=unknown",
            "#login?from=account|extra",
            "#register?from=home",
            "#registration-complete?from=home",
            "https://example.com/#login?from=home",
        ] {
            assert!(
                ProviderReturn::parse(&format!("{intent}|{hash}")).is_none(),
                "{hash}"
            );
        }
    }

    #[test]
    fn late_provider_a_response_does_not_consume_or_relabel_newer_context_b() {
        let intent_a = "a".repeat(32);
        let context_b = ProviderReturn::new(
            AuthRoute::login(ReturnDestination::MembershipFromAccount),
            "b".repeat(32),
        )
        .unwrap();
        let stored_b = context_b.serialize();
        for prefix in ["#auth-return?intent=", "#auth-cancelled?intent="] {
            assert_eq!(context_b.response(&format!("{prefix}{intent_a}")), None);
            assert_eq!(context_b.serialize(), stored_b);
            assert_eq!(ProviderReturn::parse(&stored_b), Some(context_b.clone()));
        }
        assert_eq!(
            context_b.response(&format!("#auth-return?intent={}", context_b.intent())),
            Some(ProviderReturnResponse::Completed)
        );
        assert_eq!(
            context_b.response(&format!("#auth-cancelled?intent={}", context_b.intent())),
            Some(ProviderReturnResponse::Cancelled)
        );
        assert_eq!(
            context_b.route().origin,
            ReturnDestination::MembershipFromAccount
        );
    }

    #[test]
    fn malformed_or_extra_provider_callback_fields_never_authorize_context_consumption() {
        let intent = "a".repeat(32);
        let context =
            ProviderReturn::new(AuthRoute::login(ReturnDestination::Account), &intent).unwrap();
        for hash in [
            "#auth-return".to_owned(),
            "#auth-cancelled".to_owned(),
            format!("#auth-return?intent={intent}&from=account"),
            format!("#auth-return?intent={intent}&intent={intent}"),
            format!("#auth-return?intent={intent}#account"),
            format!("#auth-return?intent=%61{}", "a".repeat(31)),
            format!("#auth-return?intent={}", "A".repeat(32)),
            format!("#auth-return?return_to=https://example.com&intent={intent}"),
            format!("#registration-complete?intent={intent}"),
            format!("https://example.com/#auth-return?intent={intent}"),
        ] {
            assert_eq!(parse_provider_callback(&hash), None, "{hash}");
            assert_eq!(context.response(&hash), None, "{hash}");
        }
    }
}
