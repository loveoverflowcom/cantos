use crate::{
    application::{
        auth_navigation::{AuthRoute, MembershipReturn, NavigationTarget},
        public_discovery::DiscoveryQuery,
    },
    core::{ids::WorkId, public_ids::PublicWorkId},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Route {
    SharedWork(PublicWorkId),
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
        if let Some(id) = value.strip_prefix("shared/") {
            return PublicWorkId::parse(id)
                .map(Self::SharedWork)
                .unwrap_or(Self::NotFound);
        }
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
            Self::PublicWork(_) | Self::SharedWork(_) => Self::NotFound,
            route => route,
        }
    }
    pub fn hash(self) -> String {
        match self {
            Self::SharedWork(id) => format!("#shared/{id}"),
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
            NavigationTarget::SharedWork(id) => Self::SharedWork(id),
            NavigationTarget::Home => Self::Discover,
            NavigationTarget::Account => Self::Account,
            NavigationTarget::Library => Self::Library,
            NavigationTarget::Auth(auth) => Self::Auth(auth),
            NavigationTarget::Membership(member) => Self::Membership(member),
        }
    }

    /// Nested routes retain their root destination without rewriting browser history.
    pub fn primary_navigation(self) -> Option<Self> {
        match self {
            Self::Discover | Self::Explore | Self::Work(_) | Self::PublicWork(_) => {
                Some(Self::Discover)
            }
            Self::SharedWork(_) | Self::Library | Self::History => Some(Self::Library),
            Self::Settings | Self::Account | Self::Membership(_) => Some(Self::Account),
            Self::Auth(auth) => Self::from_target(auth.origin.target()).primary_navigation(),
            Self::Player | Self::Queue | Self::NotFound => None,
        }
    }
}

fn link_parameters(hash: &str) -> Vec<(String, String)> {
    match hash.trim_start_matches('#').split_once('?') {
        Some((_, query)) => url::form_urlencoded::parse(query.as_bytes())
            .into_owned()
            .collect(),
        None => Vec::new(),
    }
}

/// The search a link carries; anything refused yields the unfiltered catalog.
///
/// Discovery no longer filters by content language (#424), so an older link's `language` is
/// dropped here instead of surviving as a filter the page has no control to show or clear. The
/// shell then rewrites the address (see [`retired_language_rewrite`]).
pub fn discovery_query(hash: &str) -> DiscoveryQuery {
    let pairs = link_parameters(hash);
    DiscoveryQuery::from_parameters(
        pairs
            .iter()
            .filter(|(name, _)| name != "language")
            .map(|(n, v)| (n.as_str(), v.as_str())),
    )
}

/// Whether the link still carries the retired `language` filter under any spelling the form
/// decoder reads as that name: repeated, empty, valueless or percent-encoded.
fn carries_retired_language(hash: &str) -> bool {
    link_parameters(hash)
        .iter()
        .any(|(name, _)| name == "language")
}

/// The address a discovery link shows once its retired `language` filter is dropped: the
/// canonical link of the search the page actually applies. `None` when nothing needs rewriting,
/// so reading the rewritten address again settles instead of rewriting it again.
pub fn retired_language_rewrite(route: Route, hash: &str) -> Option<String> {
    (matches!(route, Route::Discover | Route::Explore) && carries_retired_language(hash))
        .then(|| route.discovery_hash(&discovery_query(hash)))
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
        for route in [
            Route::Account,
            Route::Settings,
            Route::Membership(MembershipReturn::Account),
            Route::Membership(MembershipReturn::Home),
        ] {
            assert_eq!(route.primary_navigation(), Some(Route::Account));
        }
        use crate::application::auth_navigation::ReturnDestination;
        for (origin, family) in [
            (ReturnDestination::Home, Route::Discover),
            (ReturnDestination::Library, Route::Library),
            (ReturnDestination::Account, Route::Account),
            (ReturnDestination::Membership, Route::Account),
            (ReturnDestination::MembershipFromAccount, Route::Account),
        ] {
            for auth in [AuthRoute::login(origin), AuthRoute::register(origin)] {
                assert_eq!(Route::Auth(auth).primary_navigation(), Some(family));
            }
        }
        assert_eq!(Route::from_live_hash("#settings"), Route::Settings);
        assert_eq!(Route::from_live_hash("#saved"), Route::NotFound);
    }

    #[test]
    fn a_search_lives_in_the_discovery_link_and_round_trips_through_history() {
        use crate::contract::discovery::WorkKind;
        let query = DiscoveryQuery::new("Ánh đèn & ma", Some(WorkKind::Story), "").unwrap();
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

    #[test]
    fn a_retired_language_filter_is_dropped_from_the_query_and_flagged_for_rewrite() {
        let link = "#discover?q=%C4%91%C3%A8n&kind=story&language=vi-vn";
        let query = discovery_query(link);
        assert_eq!(query.text(), "đèn");
        assert_eq!(query.language(), None);
        assert!(carries_retired_language(link));
        // A malformed tag is no longer a reason to refuse the rest of the search.
        assert_eq!(discovery_query("#discover?q=a&language=vi_VN").text(), "a");
        assert!(!carries_retired_language("#discover?q=a&kind=story"));
        assert!(!carries_retired_language("#discover"));
        // The rewritten link is the canonical one without the filter.
        assert_eq!(
            Route::Discover.discovery_hash(&query),
            "#discover?q=%C4%91%C3%A8n&kind=story"
        );
    }

    /// Quint run: cantos.presentation.discovery-retired-language.run.retired-deep-link
    #[test]
    fn a_retired_language_link_keeps_the_rest_of_its_search_and_settles_in_one_rewrite() {
        use crate::contract::discovery::WorkKind;
        // The other parameters survive in their canonical order, decoded and re-encoded.
        let link = "#discover?language=vi-vn&kind=podcast&q=a%26b+c";
        let rewrite = retired_language_rewrite(Route::Discover, link).unwrap();
        assert_eq!(rewrite, "#discover?q=a%26b+c&kind=podcast");
        let applied = discovery_query(link);
        assert_eq!(
            applied,
            DiscoveryQuery::new("a&b c", Some(WorkKind::Podcast), "").unwrap()
        );
        // The rewritten address shows exactly the applied search, and reading it again rewrites
        // nothing: no loop of replacements.
        assert_eq!(discovery_query(&rewrite), applied);
        assert_eq!(retired_language_rewrite(Route::Discover, &rewrite), None);
        // Explore keeps its own screen.
        assert_eq!(
            retired_language_rewrite(Route::Explore, "#explore?q=x&language=en").as_deref(),
            Some("#explore?q=x")
        );
    }

    /// Quint run: cantos.presentation.discovery-retired-language.run.language-only
    #[test]
    fn a_link_that_only_carried_a_language_becomes_the_unfiltered_catalog() {
        for link in [
            "#discover?language=en",
            "#discover?language=",
            "#discover?language",
            "#discover?language=vi-vn&language=en",
            "#discover?lang%75age=en",
        ] {
            assert!(discovery_query(link).is_unfiltered(), "{link}");
            assert_eq!(
                retired_language_rewrite(Route::Discover, link).as_deref(),
                Some("#discover"),
                "{link}"
            );
        }
        // Repeated or empty values never leak the filter back, and the rest still applies.
        let link = "#discover?language=&q=x&language=en";
        assert_eq!(discovery_query(link).text(), "x");
        assert_eq!(discovery_query(link).language(), None);
        assert_eq!(
            retired_language_rewrite(Route::Discover, link).as_deref(),
            Some("#discover?q=x")
        );
        // A refused kind still yields the unfiltered catalog, and the address says so.
        assert_eq!(
            retired_language_rewrite(Route::Discover, "#discover?q=x&kind=genre&language=en")
                .as_deref(),
            Some("#discover")
        );
    }

    #[test]
    fn only_a_discovery_link_carrying_the_retired_filter_is_rewritten() {
        for (route, hash) in [
            (Route::Discover, "#discover"),
            (Route::Discover, ""),
            (Route::Discover, "#discover?"),
            (Route::Discover, "#discover?q=a&kind=story"),
            // Another name that merely contains `language` is not the retired filter.
            (Route::Discover, "#discover?languages=en&q=a"),
            (Route::Discover, "#discover?Language=en"),
        ] {
            assert_eq!(retired_language_rewrite(route, hash), None, "{hash}");
        }
        // Non-discovery screens never read a search, so they are never rewritten.
        for hash in ["#library?language=en", "#work/x?language=en", "#account"] {
            let route = Route::from_live_hash(hash);
            assert_eq!(retired_language_rewrite(route, hash), None, "{hash}");
        }
    }
}

#[cfg(test)]
mod shared_routes_tests {
    use super::*;
    use crate::application::auth_navigation::ReturnDestination;
    #[test]
    fn shared_locator_auth_return_contains_only_canonical_id_and_cannot_redirect_offsite() {
        let id = PublicWorkId::parse("10000000-0000-4000-8000-000000000001").unwrap();
        let route = Route::SharedWork(id);
        assert_eq!(Route::from_live_hash(&route.hash()), route);
        let login = AuthRoute::login(ReturnDestination::SharedWork(id));
        assert_eq!(AuthRoute::from_hash(&login.hash()), Some(login));
        assert_eq!(Route::from_target(login.origin.target()), route);
        for link in [
            "#login?from=https://evil.invalid",
            "#login?from=shared/private-title",
            "#login?from=shared/10000000-0000-4000-8000-000000000001&title=secret",
        ] {
            assert_eq!(
                AuthRoute::from_hash(link).unwrap().origin,
                ReturnDestination::Home
            );
        }
        assert_eq!(
            Route::from_live_hash("#shared/private-title"),
            Route::NotFound
        );
        assert_eq!(Route::from_mock_hash(&route.hash()), Route::NotFound);
    }
}
