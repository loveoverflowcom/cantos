use crate::{
    application::auth_navigation::{AuthRoute, MembershipReturn, NavigationTarget},
    core::{ids::WorkId, public_ids::PublicWorkId},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Route {
    Discover,
    Explore,
    /// Isolated mock fixture detail; only the `?mode=mock` shell renders it.
    Work(WorkId),
    /// Live public detail keyed by the server-issued canonical UUID; the mock shell refuses it.
    PublicWork(PublicWorkId),
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
            value => match value.strip_prefix("work/") {
                Some(key) => WorkId::from_slug(key)
                    .map(Self::Work)
                    .or_else(|| PublicWorkId::parse(key).map(Self::PublicWork))
                    .unwrap_or(Self::NotFound),
                None => Self::NotFound,
            },
        }
    }
    /// The isolated mock shell has no live catalog: a public ID is an unknown route there.
    pub fn from_mock_hash(hash: &str) -> Self {
        match Self::from_hash(hash) {
            Self::PublicWork(_) => Self::NotFound,
            route => route,
        }
    }
    pub fn hash(self) -> String {
        match self {
            Self::Discover => "#discover".into(),
            Self::Explore => "#explore".into(),
            Self::Work(id) => format!("#work/{}", id.slug()),
            Self::PublicWork(id) => format!("#work/{id}"),
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
            Self::Discover | Self::Explore | Self::Work(_) | Self::PublicWork(_) => {
                Some(Self::Discover)
            }
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
    fn public_work_route_admits_only_a_canonical_server_id_and_keeps_mock_slugs_separate() {
        let canonical = "10000000-0000-4000-8000-00000000000a";
        let id = PublicWorkId::parse(canonical).unwrap();
        let route = Route::from_hash(&format!("#work/{canonical}"));
        assert_eq!(route, Route::PublicWork(id));
        assert_eq!(route.hash(), format!("#work/{canonical}"));
        assert_eq!(Route::from_hash(&route.hash()), route);
        assert_eq!(route.primary_navigation(), Some(Route::Discover));
        assert_eq!(
            Route::from_hash("#work/son-ha-ky"),
            Route::Work(WorkId::Mountain)
        );
        assert_eq!(Route::from_mock_hash(&route.hash()), Route::NotFound);
        assert_eq!(
            Route::from_mock_hash("#work/den-khuya"),
            Route::Work(WorkId::River)
        );
        for rejected in [
            "#work/10000000-0000-4000-8000-00000000000A",
            "#work/{10000000-0000-4000-8000-00000000000a}",
            "#work/1000000000004000800000000000000a",
            "#work/10000000-0000-4000-8000-00000000000a/units",
            "#work/",
        ] {
            assert_eq!(Route::from_hash(rejected), Route::NotFound, "{rejected}");
        }
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
