//! Developer-only catalogue UI; registration never implies production qualification.

use super::token_sample::TokenSample;
use super::{catalog, href, name, preview_catalog};
use cantos_ui::{
    Button, ButtonVariant, Grid, GridColumns, Link, Select, Space, Stack, Surface, SurfaceTone,
    TextField, TextFieldKind,
};
use leptos::prelude::*;

#[component]
pub fn ComponentCatalog(
    query: RwSignal<String>,
    family: RwSignal<&'static str>,
    current_anchor: RwSignal<String>,
) -> impl IntoView {
    view! {
        <Surface tone=SurfaceTone::Container>
            <section id="gallery-catalog" aria-labelledby="gallery-catalog-heading">
                <Stack>
                    <h2 id="gallery-catalog-heading">"Danh mục component đã khai báo"</h2>
                    <p>"Tìm theo API, nhóm hoặc trạng thái. Bộ lọc chỉ ẩn vùng mẫu; văn bản, lựa chọn và số lần thao tác vẫn được giữ."</p>
                    <Grid columns=GridColumns::Two>
                        <TextField id="gallery-catalog-query" label="Tìm component" kind=TextFieldKind::Search
                            value=query placeholder="Ví dụ: TextField, loading, dialog"
                            help=Some("Các từ cùng phải khớp; không phân biệt chữ hoa/thường. Chỉ tìm trong danh mục cục bộ.".to_owned())
                            on_input=Callback::new(move |value| query.set(value))/>
                        <Select id="gallery-catalog-family" label="Nhóm component" value=Signal::derive(move || family.get().to_owned())
                            on_input=Callback::new(move |value: String| family.set(catalog::family_key(&value)))>
                            <option value="all" selected=move || family.get() == "all">"Tất cả nhóm"</option>
                            {catalog::FAMILIES.iter().map(|entry| view! {
                                <option value=entry.key selected=move || family.get() == entry.key>{entry.label}</option>
                            }).collect_view()}
                        </Select>
                    </Grid>
                    <Button id="gallery-catalog-reset" variant=ButtonVariant::Text
                        on_press=Callback::new(move |_| { query.set(String::new()); family.set("all"); })>"Hiển thị tất cả mẫu"</Button>
                    <p id="gallery-catalog-count" class="cui-gallery-note" role="status" aria-live="polite" aria-atomic="true">
                        {move || format!("{} / {} component khớp. Trạng thái đăng ký là bằng chứng theo renderer, không phải chứng nhận runtime.", preview_catalog::COMPONENTS.iter().filter(|entry| catalog::component_matches(entry, family.get(), &query.get())).count(), preview_catalog::COMPONENTS.len())}
                    </p>
                    <p id="gallery-catalog-empty" hidden=move || preview_catalog::COMPONENTS.iter().any(|entry| catalog::component_matches(entry, family.get(), &query.get()))>
                        "Không có component khớp. Xóa từ tìm hoặc chọn tất cả nhóm để xem lại các mẫu."
                    </p>
                    <details>
                        <summary>"API, trạng thái mẫu và tình trạng từng renderer"</summary>
                        <ul class="cui-catalog-entries">
                            {preview_catalog::COMPONENTS.iter().map(|entry| {
                                let anchor = catalog::sample_anchor(entry.family).expect("shared registrations have preview sample families");
                                view! {
                                    <li id=format!("gallery-registration-{}", entry.id) class="cui-catalog-entry"
                                        data-component=entry.id data-family=entry.family
                                        hidden=move || !catalog::component_matches(entry, family.get(), &query.get())>
                                        <h3><Link id=format!("gallery-registration-link-{}", entry.id) href=href(&format!("#{anchor}"))
                                            on_press=Callback::new(move |_| { family.set(entry.family); query.set(String::new()); current_anchor.set(entry.family.to_owned()); })>{entry.name}</Link></h3>
                                        <dl class="cui-catalog-meta">
                                            <div><dt>"ID / nhóm"</dt><dd><code>{entry.id}</code>" · "{entry.family}</dd></div>
                                            <div><dt>"Trạng thái mẫu"</dt><dd>{entry.states.join(" · ")}</dd></div>
                                            <div><dt>"Leptos"</dt><dd><code>{entry.leptos.status}</code>" · "{entry.leptos.reason}</dd></div>
                                            <div><dt>"CMP"</dt><dd><code>{entry.cmp.status}</code>" · "{entry.cmp.reason}</dd></div>
                                        </dl>
                                    </li>
                                }
                            }).collect_view()}
                        </ul>
                    </details>
                </Stack>
            </section>
        </Surface>
    }
}

#[component]
pub fn TokenCatalog(#[prop(default = "")] initial_query: &'static str) -> impl IntoView {
    let query = RwSignal::new(initial_query.to_owned());
    view! {
        <section id="gallery-tokens" aria-labelledby="gallery-tokens-heading">
            <Stack gap=Space::Sm>
                <h2 id="gallery-tokens-heading">"Token có tên từ nguồn chung"</h2>
                <p class="cui-gallery-note">{format!("Nguồn token v{}. Giá trị CSS chỉ đọc; mở một tên để so sánh hai theme và ghi đè mật độ. Không tạo palette riêng cho phòng mẫu.", preview_catalog::TOKEN_VERSION)}</p>
                <TextField id="gallery-token-query" label="Tìm token" kind=TextFieldKind::Search value=query
                    placeholder="Ví dụ: color.primary, shape, 3rem"
                    on_input=Callback::new(move |value| query.set(value))/>
                <p class="cui-gallery-note" id="gallery-token-count" role="status" aria-live="polite" aria-atomic="true">
                    {move || format!("{} / {} token khớp", preview_catalog::TOKENS.iter().filter(|entry| catalog::token_matches(entry, &query.get())).count(), preview_catalog::TOKENS.len())}
                </p>
                <p id="gallery-token-empty" hidden=move || preview_catalog::TOKENS.iter().any(|entry| catalog::token_matches(entry, &query.get()))>
                    "Không có token khớp. Xóa từ tìm để xem toàn bộ nguồn chung."
                </p>
                <ul class="cui-token-entries">
                    {preview_catalog::TOKENS.iter().map(|entry| view! {
                        <li class="cui-catalog-entry" data-token=entry.name hidden=move || !catalog::token_matches(entry, &query.get())>
                            <details>
                                <summary><code>{entry.name}</code>" · "{entry.kind}</summary>
                                <TokenSample token=entry/>
                                <dl class="cui-catalog-meta">
                                    <div><dt>"Biến CSS"</dt><dd><code>{entry.css_variable}</code></dd></div>
                                    <div><dt>"Sáng"</dt><dd><code>{entry.light}</code></dd></div>
                                    <div><dt>"Tối"</dt><dd><code>{entry.dark}</code></dd></div>
                                    <div><dt>"Mật độ gọn"</dt><dd><code>{entry.compact.unwrap_or("Không ghi đè; dùng giá trị theme")}</code></dd></div>
                                    <div><dt>"Mật độ thoải mái"</dt><dd><code>{entry.comfortable.unwrap_or("Không ghi đè; dùng giá trị theme")}</code></dd></div>
                                    <div><dt>"Alias"</dt><dd><code>{entry.alias_of.unwrap_or("Vai trò gốc")}</code></dd></div>
                                </dl>
                            </details>
                        </li>
                    }).collect_view()}
                </ul>
            </Stack>
        </section>
    }
}

#[component]
pub fn FamilyNavigation(
    current_anchor: RwSignal<String>,
    family: RwSignal<&'static str>,
    query: RwSignal<String>,
) -> impl IntoView {
    use cantos_ui::{NavItem, NavTabs};
    view! {
        <NavTabs label=name("Các nhóm component trong phòng mẫu")>
            {catalog::FAMILIES.iter().map(|entry| view! {
                <NavItem id=format!("gallery-nav-{}", entry.key) href=href(&format!("#{}", entry.anchor)) icon=entry.icon
                    current=Signal::derive(move || current_anchor.get() == entry.key)
                    on_press=Callback::new(move |_| { family.set(entry.key); query.set(String::new()); current_anchor.set(entry.key.to_owned()); })>{entry.label}</NavItem>
            }).collect_view()}
        </NavTabs>
    }
}
