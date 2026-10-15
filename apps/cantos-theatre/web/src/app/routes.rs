use crate::{
    application::auth_navigation::{AuthRoute, MembershipReturn, NavigationTarget},
    core::ids::WorkId,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Route {
    Discover,
    Explore,
    Work(WorkId),
    Library,
    History,
    Player,
    Queue,
    Settings,
    Account,
    Auth(AuthRoute),
    Membership(MembershipReturn),
    NotFound,
}
impl Route {
    pub fn from_hash(hash: &str) -> Self {
        if let Some(auth) = AuthRoute::from_hash(hash) {
            return Self::Auth(auth);
        }
        if let Some(member) = MembershipReturn::from_hash(hash) {
            return Self::Membership(member);
        }
        match hash.trim_start_matches('#') {
            "" | "discover" => Self::Discover,
            "explore" => Self::Explore,
            "library" => Self::Library,
            "history" => Self::History,
            "player" => Self::Player,
            "queue" => Self::Queue,
            "settings" => Self::Settings,
            "account" => Self::Account,
            value => value
                .strip_prefix("work/")
                .and_then(WorkId::from_slug)
                .map(Self::Work)
                .unwrap_or(Self::NotFound),
        }
    }
    pub fn hash(self) -> String {
        match self {
            Self::Discover => "#discover".into(),
            Self::Explore => "#explore".into(),
            Self::Work(id) => format!("#work/{}", id.slug()),
            Self::Library => "#library".into(),
            Self::History => "#history".into(),
            Self::Player => "#player".into(),
            Self::Queue => "#queue".into(),
            Self::Settings => "#settings".into(),
            Self::Account => "#account".into(),
            Self::Auth(auth) => auth.hash(),
            Self::Membership(member) => member.hash(),
            Self::NotFound => "#not-found".into(),
        }
    }

    pub fn from_target(target: NavigationTarget) -> Self {
        match target {
            NavigationTarget::Home => Self::Discover,
            NavigationTarget::Account => Self::Account,
            NavigationTarget::Auth(auth) => Self::Auth(auth),
            NavigationTarget::Membership(member) => Self::Membership(member),
        }
    }

    /// Detail and history routes belong to one of the two main navigation destinations.
    pub fn primary_navigation(self) -> Option<Self> {
        match self {
            Self::Discover | Self::Explore | Self::Work(_) => Some(Self::Discover),
            Self::Library | Self::History => Some(Self::Library),
            Self::Player
            | Self::Queue
            | Self::Settings
            | Self::Account
            | Self::Auth(_)
            | Self::Membership(_)
            | Self::NotFound => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn routes_round_trip_and_unknown_values_have_a_recovery_route() {
        for route in [
            Route::Discover,
            Route::Explore,
            Route::Library,
            Route::History,
            Route::Player,
            Route::Queue,
            Route::Settings,
            Route::Account,
            Route::NotFound,
        ] {
            assert_eq!(Route::from_hash(&route.hash()), route);
        }
        for work in WorkId::ALL {
            assert_eq!(
                Route::from_hash(&Route::Work(work).hash()),
                Route::Work(work)
            );
        }
        assert_eq!(Route::from_hash("#work/secret-draft"), Route::NotFound);
    }

    #[test]
    fn nested_routes_keep_their_primary_destination() {
        assert_eq!(Route::Explore.primary_navigation(), Some(Route::Discover));
        for work in WorkId::ALL {
            assert_eq!(
                Route::Work(work).primary_navigation(),
                Some(Route::Discover)
            );
        }
        assert_eq!(Route::History.primary_navigation(), Some(Route::Library));
        assert_eq!(Route::Account.primary_navigation(), None);
    }
}
