use crate::core::ids::WorkId;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Route { Discover, Explore, Work(WorkId), Library, Player, Queue, Settings, Account, NotFound }
impl Route {
    pub fn from_hash(hash: &str) -> Self {
        match hash.trim_start_matches('#') {
            "" | "discover" => Self::Discover, "explore" => Self::Explore,
            "library" => Self::Library, "player" => Self::Player, "queue" => Self::Queue,
            "settings" => Self::Settings, "account" => Self::Account,
            value => value.strip_prefix("work/").and_then(WorkId::from_slug).map(Self::Work).unwrap_or(Self::NotFound),
        }
    }
    pub fn hash(self) -> String {
        match self {
            Self::Discover => "#discover".into(), Self::Explore => "#explore".into(),
            Self::Work(id) => format!("#work/{}", id.slug()), Self::Library => "#library".into(),
            Self::Player => "#player".into(), Self::Queue => "#queue".into(),
            Self::Settings => "#settings".into(), Self::Account => "#account".into(), Self::NotFound => "#not-found".into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn routes_round_trip_and_unknown_values_have_a_recovery_route() {
        for route in [Route::Discover, Route::Explore, Route::Library, Route::Player, Route::Queue, Route::Settings, Route::Account, Route::NotFound] { assert_eq!(Route::from_hash(&route.hash()), route); }
        for work in WorkId::ALL { assert_eq!(Route::from_hash(&Route::Work(work).hash()), Route::Work(work)); }
        assert_eq!(Route::from_hash("#work/secret-draft"), Route::NotFound);
    }
}
