use crate::{
    application::{
        auth_navigation::{AuthRoute, MembershipReturn, NavigationTarget},
        public_discovery::DiscoveryQuery,
    },
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
        let value = hash.trim_start_matches('#');
        // Only the two discovery screens carry a search in their link.
        if let Some((path, _)) = value.split_once('?') {
            return match path {
                "discover" => Self::Discover,
                "explore" => Self::Explore,
                _ => Self::NotFound,
            };
        }
        match value {
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
    /// The live shell has no mock fixtures: a sample slug is an unknown route there.
    pub fn from_live_hash(hash: &str) -> Self {
        match Self::from_hash(hash) {
            Self::Work(_) => Self::NotFound,
            route => route,
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

    /// The link of a discovery screen carrying its search; an unfiltered search is the bare route.
    pub fn discovery_hash(self, query: &DiscoveryQuery) -> String {
        let base = match self {
            Self::Explore => "#explore",
            _ => "#discover",
        };
        let parameters = query.parameters();
        if parameters.is_empty() {
            return base.to_owned();
        }
        let mut serializer = url::form_urlencoded::Serializer::new(String::new());
        for (name, value) in parameters {
            serializer.append_pair(name, &value);
        }
        format!("{base}?{}", serializer.finish())
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

/// The search a link carries; anything refused yields the unfiltered catalog.
pub fn discovery_query(hash: &str) -> DiscoveryQuery {
    match hash.trim_start_matches('#').split_once('?') {
        Some((_, query)) => {
            let pairs: Vec<(String, String)> = url::form_urlencoded::parse(query.as_bytes())
                .into_owned()
                .collect();
            DiscoveryQuery::from_parameters(pairs.iter().map(|(n, v)| (n.as_str(), v.as_str())))
        }
        None => DiscoveryQuery::default(),
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

    #[test]
    fn a_search_lives_in_the_discovery_link_and_round_trips_through_history() {
        use cantos_contracts::theatre::WorkKind;
        let query = DiscoveryQuery::new("Ánh đèn & ma", Some(WorkKind::Story), "vi-VN").unwrap();
        for route in [Route::Discover, Route::Explore] {
            let hash = route.discovery_hash(&query);
            assert_eq!(Route::from_hash(&hash), route);
            assert_eq!(discovery_query(&hash), query);
            assert!(hash.starts_with(&route.hash()));
        }
        assert_eq!(
            Route::Discover.discovery_hash(&DiscoveryQuery::default()),
            "#discover"
        );
        assert_eq!(discovery_query("#discover"), DiscoveryQuery::default());
        assert_eq!(discovery_query(""), DiscoveryQuery::default());
        // A refused filter never half-applies; unknown parameters are ignored.
        assert_eq!(
            discovery_query("#discover?q=a&kind=genre"),
            DiscoveryQuery::default()
        );
        assert_eq!(discovery_query("#discover?x=1&q=%C4%91").text(), "đ");
        // Other routes do not accept a query string.
        for hash in ["#library?q=a", "#player?x", "#work/secret?q=a", "#queue?"] {
            assert_eq!(Route::from_hash(hash), Route::NotFound, "{hash}");
        }
    }

    #[test]
    fn the_live_shell_does_not_resolve_mock_fixture_slugs() {
        for work in WorkId::ALL {
            let hash = Route::Work(work).hash();
            assert_eq!(Route::from_live_hash(&hash), Route::NotFound, "{hash}");
            assert_eq!(Route::from_mock_hash(&hash), Route::Work(work));
        }
        let public = "#work/10000000-0000-4000-8000-00000000000a";
        assert!(matches!(
            Route::from_live_hash(public),
            Route::PublicWork(_)
        ));
        assert_eq!(Route::from_live_hash("#player"), Route::Player);
    }
}
