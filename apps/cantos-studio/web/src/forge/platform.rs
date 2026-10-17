//! App-owned focus after a confirmed mock operation disables its modal opener.
use wasm_bindgen::JsCast;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FocusFailure {
    MissingTarget,
    Rejected,
}

pub fn focus_step() -> Result<(), FocusFailure> {
    let document = web_sys::window()
        .and_then(|w| w.document())
        .ok_or(FocusFailure::MissingTarget)?;
    let element = document
        .get_element_by_id("forge-step-title")
        .and_then(|element| element.dyn_into::<web_sys::HtmlElement>().ok())
        .ok_or(FocusFailure::MissingTarget)?;
    element.focus().map_err(|_| FocusFailure::Rejected)?;
    if document
        .active_element()
        .is_some_and(|active| active.is_same_node(Some(&element)))
    {
        Ok(())
    } else {
        Err(FocusFailure::Rejected)
    }
}
