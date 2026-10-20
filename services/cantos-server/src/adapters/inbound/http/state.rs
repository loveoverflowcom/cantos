//! Request state; concrete capabilities are assembled separately.
use crate::application::Store;

#[derive(Clone)]
pub struct AppState {
    pub store: Store,
    pub web_origin: String,
}
