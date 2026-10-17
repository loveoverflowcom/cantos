//! Official local artwork in paired surfaces; no app adoption or remote asset fixture.
use super::messages::tool;
use cantos_ui::{
    AccessibleName, Grid, GridColumns, Logo, LogoTone, LogoVariant, Space, Stack, Surface,
    SurfaceTone, Theme, UiRoot,
};
use leptos::prelude::*;

#[component]
pub fn LogoSamples() -> impl IntoView {
    view! {
        <div lang=tool::LANGUAGE_TAG data-preview-component="logo"
            data-preview-scenario="cantos-light cantos-dark cantos-mono studio-light studio-dark studio-mono symbol-light symbol-dark symbol-mono decorative named aspect-fit transparent-knockout">
            <Stack gap=Space::Sm>
                <h3>{tool::LOGO_HEADING}</h3>
                <p class="cui-gallery-note">{tool::LOGO_HELP}</p>
                <Grid columns=GridColumns::Two>
                    {LogoVariant::ALL.iter().copied().flat_map(|variant| {
                        LogoTone::ALL.iter().copied().map(move |tone| {
                            let theme = if tone == LogoTone::Dark { Theme::Dark } else { Theme::Light };
                            let surface = if tone == LogoTone::Mono { SurfaceTone::PrimaryContainer } else { SurfaceTone::Plain };
                            let label = match variant {
                                LogoVariant::Cantos => "Cantos",
                                LogoVariant::CantosStudio => "Cantos Studio",
                                LogoVariant::Symbol => tool::LOGO_SYMBOL,
                            };
                            let tone_label = match tone {
                                LogoTone::Light => tool::LOGO_LIGHT,
                                LogoTone::Dark => tool::LOGO_DARK,
                                LogoTone::Mono => tool::LOGO_MONO,
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
                        <p>{tool::LOGO_DECORATIVE}</p>
                    </Stack>
                </div>
                <div lang="vi-VN" data-preview-fixture="vietnamese" data-logo-state="named">
                    <p lang=tool::LANGUAGE_TAG>{tool::LOGO_NAMED}</p>
                    <Logo variant=LogoVariant::Symbol
                        accessible_name=AccessibleName::new("Biểu tượng Cantos").expect("preview name")/>
                </div>
                <div data-logo-state="aspect-fit" style="max-inline-size: 140px">
                    <p>{tool::LOGO_ASPECT}</p>
                    <Logo variant=LogoVariant::CantosStudio/>
                </div>
                </Surface>
                </UiRoot>
            </Stack>
        </div>
    }
}
