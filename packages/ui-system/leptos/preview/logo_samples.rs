//! Official local artwork in paired surfaces; no app adoption or remote asset fixture.
use cantos_ui::{
    AccessibleName, Grid, GridColumns, Logo, LogoTone, LogoVariant, Space, Stack, Surface,
    SurfaceTone, Theme, UiRoot,
};
use leptos::prelude::*;

#[component]
pub fn LogoSamples() -> impl IntoView {
    view! {
        <div data-preview-component="logo"
            data-preview-scenario="cantos-light cantos-dark cantos-mono studio-light studio-dark studio-mono symbol-light symbol-dark symbol-mono decorative named aspect-fit transparent-knockout">
            <Stack gap=Space::Sm>
                <h3>"Logo chính thức · vector nội bộ"</h3>
                <p class="cui-gallery-note">"Chữ đã là path. Mỗi nền bên dưới thuộc bản mẫu, không được vẽ vào logo. Bản đơn sắc giữ khoảng âm trong suốt."</p>
                <Grid columns=GridColumns::Two>
                    {LogoVariant::ALL.iter().copied().flat_map(|variant| {
                        LogoTone::ALL.iter().copied().map(move |tone| {
                            let theme = if tone == LogoTone::Dark { Theme::Dark } else { Theme::Light };
                            let surface = if tone == LogoTone::Mono { SurfaceTone::PrimaryContainer } else { SurfaceTone::Plain };
                            let label = match variant {
                                LogoVariant::Cantos => "Cantos",
                                LogoVariant::CantosStudio => "Cantos Studio",
                                LogoVariant::Symbol => "Biểu tượng",
                            };
                            let tone_label = match tone {
                                LogoTone::Light => "nền sáng",
                                LogoTone::Dark => "nền tối",
                                LogoTone::Mono => "đơn sắc / khoảng âm trong suốt",
                            };
                            view! {
                                <UiRoot theme=theme>
                                    <Surface tone=surface>
                                        <Stack gap=Space::Md>
                                            <p>{format!("{label} · {tone_label}")}</p>
                                            <Logo variant=variant tone=tone/>
                                        </Stack>
                                    </Surface>
                                </UiRoot>
                            }
                        })
                    }).collect_view()}
                </Grid>
                // The following examples use Light artwork; keep their surface paired even
                // while the gallery host reviews a dark recipe.
                <UiRoot theme=Theme::Light>
                <Surface tone=SurfaceTone::Plain>
                <div data-logo-state="decorative">
                    <Stack direction=cantos_ui::StackDirection::Horizontal gap=Space::Md>
                        <Logo variant=LogoVariant::Symbol/>
                        <p>"Cantos · biểu tượng trang trí bên cạnh tên"</p>
                    </Stack>
                </div>
                <div data-logo-state="named">
                    <p>"Biểu tượng độc lập có tên cho công nghệ hỗ trợ"</p>
                    <Logo variant=LogoVariant::Symbol
                        accessible_name=AccessibleName::new("Biểu tượng Cantos").expect("preview name")/>
                </div>
                <div data-logo-state="aspect-fit" style="max-inline-size: 140px">
                    <p>"Khung hẹp vẫn giữ đúng tỷ lệ"</p>
                    <Logo variant=LogoVariant::CantosStudio/>
                </div>
                </Surface>
                </UiRoot>
            </Stack>
        </div>
    }
}
