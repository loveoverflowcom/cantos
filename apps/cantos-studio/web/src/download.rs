//! Private original download: authenticated byte fetch, then an ephemeral local download.
use crate::ui::ActionButton;
use crate::{api::StudioContext, messages};
use cantos_ui::ButtonVariant;
use leptos::prelude::*;
use wasm_bindgen::JsCast;

#[component]
pub fn DownloadOriginal(
    namespace: &'static str,
    id: String,
    file_name: String,
    sha256: String,
    locale: Signal<crate::localization::Locale>,
    #[prop(optional)] blocked: Option<Signal<bool>>,
) -> impl IntoView {
    let api = expect_context::<StudioContext>().api;
    let id = StoredValue::new(id);
    let file_name = StoredValue::new(file_name);
    let sha256 = StoredValue::new(sha256);
    let busy = RwSignal::new(false);
    let failure = RwSignal::new(None::<crate::contract::ApiError>);
    view! {
        <div class="download-original">
            <ActionButton id=format!("download-{namespace}-{}",id.get_value()) locale variant=ButtonVariant::Text
                blocked=Signal::derive(move ||blocked.is_some_and(|blocked|blocked.get())) busy=Signal::derive(move ||busy.get())
                on_press=Callback::new(move |_| {
                    if busy.get_untracked() || blocked.is_some_and(|blocked|blocked.get_untracked()) {return;}
                    busy.set(true);failure.set(None);
                    let api=api.snapshot();let id=id.get_value();let file_name=file_name.get_value();let sha256=sha256.get_value();
                    leptos::task::spawn_local(async move {
                        let result=async {
                            let (bytes,credential)=api.download_original(&id).await?;
                            save(bytes,&file_name,&sha256,credential).await
                        }.await;
                        let _=failure.try_set(result.err());let _=busy.try_set(false);
                    });
                })>{move || messages::import_copy(locale.get()).download}</ActionButton>
            <p class="status" role="status" aria-live="polite">{move || failure.get().map(|error|messages::status(crate::editor::error_status(&error.code),locale.get()))}</p>
        </div>
    }
}
async fn save(
    bytes: Vec<u8>,
    file_name: &str,
    expected_hash: &str,
    credential: cantos_web_auth::browser::Lease,
) -> Result<(), crate::contract::ApiError> {
    let error = || crate::api::auth_error(cantos_web_auth::Failure::Protocol);
    let window = web_sys::window().ok_or_else(error)?;
    let promise = window
        .crypto()
        .map_err(|_| error())?
        .subtle()
        .digest_with_str_and_u8_array("SHA-256", &bytes)
        .map_err(|_| error())?;
    let digest = wasm_bindgen_futures::JsFuture::from(promise)
        .await
        .map_err(|_| error())?;
    let digest: String = js_sys::Uint8Array::new(&digest)
        .to_vec()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    if digest != expected_hash {
        return Err(error());
    }
    credential.check().map_err(crate::api::auth_error)?;
    let parts = js_sys::Array::new();
    parts.push(&js_sys::Uint8Array::from(bytes.as_slice()));
    let blob = web_sys::Blob::new_with_u8_array_sequence(&parts).map_err(|_| error())?;
    let url = web_sys::Url::create_object_url_with_blob(&blob).map_err(|_| error())?;
    let result = (|| {
        let document = window.document().ok_or_else(error)?;
        let anchor = document
            .create_element("a")
            .map_err(|_| error())?
            .dyn_into::<web_sys::HtmlAnchorElement>()
            .map_err(|_| error())?;
        anchor.set_href(&url);
        anchor.set_download(file_name);
        anchor.click();
        Ok(())
    })();
    let _ = web_sys::Url::revoke_object_url(&url);
    result
}
