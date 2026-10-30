//! Browser measurements/effects for Theatre-owned navigation; the layout decision is pure Rust.
use leptos::{html, prelude::*};

pub(crate) fn bind(
    dialog: NodeRef<html::Dialog>,
    menu: NodeRef<html::Button>,
    rail_toggle: NodeRef<html::Button>,
    close: NodeRef<html::Button>,
    shell: NodeRef<html::Div>,
    route: ReadSignal<crate::app::routes::Route>,
) {
    #[cfg(target_arch = "wasm32")]
    {
        use leptos::wasm_bindgen::closure::Closure;
        type Decide = Closure<dyn FnMut(f64, bool, bool, bool) -> String>;
        let binding = StoredValue::new_local(None::<(NavigationDrawer, Decide)>);
        Effect::new(move |_| {
            let (Some(dialog), Some(menu), Some(toggle), Some(close), Some(shell)) = (
                dialog.get(),
                menu.get(),
                rail_toggle.get(),
                close.get(),
                shell.get(),
            ) else {
                return;
            };
            binding.update_value(|current| {
                if let Some((previous, _)) = current.take() {
                    previous.dispose();
                }
                let decide = Decide::new(|width, collapsed, compact_fits, expanded_fits| {
                    crate::application::navigation_layout::navigation_layout(
                        width,
                        collapsed,
                        compact_fits,
                        expanded_fits,
                    )
                    .as_str()
                    .to_owned()
                });
                let mounted = mount_navigation_drawer(
                    &dialog,
                    &menu,
                    &toggle,
                    &close,
                    &shell,
                    decide.as_ref(),
                );
                *current = Some((mounted, decide));
            });
        });
        Effect::new(move |_| {
            route.track();
            binding.with_value(|current| {
                if let Some((current, _)) = current {
                    current.route_changed();
                }
            });
        });
        on_cleanup(move || {
            binding.try_update_value(|current| {
                if let Some((current, _)) = current.take() {
                    current.dispose();
                }
            });
        });
    }
    #[cfg(not(target_arch = "wasm32"))]
    let _ = (dialog, menu, rail_toggle, close, shell, route);
}

#[cfg(target_arch = "wasm32")]
#[leptos::wasm_bindgen::prelude::wasm_bindgen(
    wasm_bindgen = leptos::wasm_bindgen,
    inline_js = r#"
export function mount_navigation_drawer(dialog, headerMenu, railToggle, closeButton, shell, decide) {
    const doc = dialog.ownerDocument;
    const win = doc.defaultView;
    const listeners = [];
    let mode = null;
    let wideCollapsed = false;
    let opener = null;
    let overflow = null;
    let disposed = false;
    const visible = element => element?.isConnected && element.getClientRects().length > 0;
    const focus = element => { if (visible(element)) element.focus({preventScroll: true}); };
    const listen = (target, type, callback) => {
        target.addEventListener(type, callback);
        listeners.push(() => target.removeEventListener(type, callback));
    };
    const updateControls = () => {
        const inlineToggle = win.innerWidth >= 1200 && mode !== 'drawer';
        railToggle.setAttribute('aria-controls', inlineToggle ? 'theatre-navigation-rail' : dialog.id);
        railToggle.setAttribute('aria-expanded', String(inlineToggle ? mode === 'expanded' : dialog.open));
        if (inlineToggle) railToggle.removeAttribute('aria-haspopup');
        else railToggle.setAttribute('aria-haspopup', 'dialog');
        const action = inlineToggle ? (mode === 'expanded' ? 'collapse' : 'expand') : 'menu';
        railToggle.dataset.action = action;
        railToggle.querySelectorAll('[data-toggle-label]').forEach(label => {
            label.hidden = label.dataset.toggleLabel !== action;
        });
        headerMenu.setAttribute('aria-expanded', String(dialog.open));
    };
    const unlock = () => {
        if (overflow !== null) {
            if (overflow.value) doc.body.style.setProperty('overflow', overflow.value, overflow.priority);
            else doc.body.style.removeProperty('overflow');
            overflow = null;
        }
        updateControls();
    };
    const close = (restore = true) => {
        if (dialog.open) dialog.close();
        unlock();
        if (restore) focus(opener);
    };
    const focusDestination = () => {
        const current = shell.querySelector('#theatre-navigation-rail [aria-current="page"]');
        if (visible(current)) focus(current);
        else if (visible(headerMenu)) focus(headerMenu);
        else focus(doc.getElementById('theatre-main'));
    };
    const fits = kind => [...shell.querySelectorAll(`[data-navigation-fit="${kind}"]`)]
        .every(label => label.scrollWidth <= label.clientWidth + 0.5);
    const reconcile = () => {
        if (disposed) return;
        const next = decide(win.innerWidth, wideCollapsed, fits('compact'), fits('expanded'));
        if (next !== mode) {
            const wasOpen = dialog.open;
            const active = doc.activeElement;
            const navigationHadFocus = active === headerMenu || shell.querySelector('#theatre-navigation-rail')?.contains(active);
            mode = next;
            shell.dataset.navigation = mode;
            if (wasOpen) close(false);
            if (wasOpen || (navigationHadFocus && !visible(active))) focusDestination();
        }
        updateControls();
    };
    const open = trigger => {
        if (disposed || dialog.open || !visible(trigger)) return;
        opener = trigger;
        focus(trigger);
        try { dialog.showModal(); } catch { unlock(); return; }
        overflow = {value: doc.body.style.getPropertyValue('overflow'), priority: doc.body.style.getPropertyPriority('overflow')};
        doc.body.style.setProperty('overflow', 'hidden');
        updateControls();
        focus(dialog.querySelector('[aria-current="page"]') || closeButton);
    };
    listen(headerMenu, 'click', () => open(headerMenu));
    listen(railToggle, 'click', () => {
        if (win.innerWidth >= 1200 && mode !== 'drawer') {
            wideCollapsed = !wideCollapsed;
            reconcile();
            focus(railToggle);
        } else open(railToggle);
    });
    listen(closeButton, 'click', () => close());
    listen(dialog, 'cancel', event => { event.preventDefault(); close(); });
    // A queued close event belongs to the previous opening, not a rapid reopen.
    listen(dialog, 'close', () => { if (!dialog.open) unlock(); });
    listen(dialog, 'click', event => {
        const link = event.target.closest('a[href]');
        if (link) {
            const currentAddress = link.hash === win.location.hash;
            if (currentAddress) event.preventDefault();
            close(false);
            focus(doc.getElementById('theatre-main'));
            return;
        }
        if (event.target !== dialog) return;
        const rect = dialog.getBoundingClientRect();
        if (event.clientX < rect.left || event.clientX > rect.right || event.clientY < rect.top || event.clientY > rect.bottom) close();
    });
    listen(dialog, 'keydown', event => {
        if (event.key !== 'Tab') return;
        const targets = [...dialog.querySelectorAll('button:not([disabled]), a[href]')].filter(visible);
        const first = targets[0], last = targets[targets.length - 1];
        if (event.shiftKey && doc.activeElement === first) { event.preventDefault(); focus(last); }
        else if (!event.shiftKey && doc.activeElement === last) { event.preventDefault(); focus(first); }
    });
    const routeChanged = () => { if (dialog.open) { close(false); focus(doc.getElementById('theatre-main')); } };
    listen(win, 'hashchange', routeChanged);
    listen(win, 'popstate', routeChanged);
    listen(win, 'resize', reconcile);
    if (win.visualViewport) listen(win.visualViewport, 'resize', reconcile);
    const probes = shell.querySelector('.theatre-navigation-fit');
    const resize = new ResizeObserver(reconcile);
    probes.querySelectorAll('[data-navigation-fit]').forEach(label => resize.observe(label));
    const text = new MutationObserver(reconcile);
    text.observe(probes, {childList: true, characterData: true, subtree: true});
    if (doc.fonts) {
        listen(doc.fonts, 'loadingdone', reconcile);
        doc.fonts.ready.then(() => { if (!disposed) reconcile(); });
    }
    reconcile();
    return {route_changed: routeChanged, dispose() {
        disposed = true;
        close(false);
        resize.disconnect();
        text.disconnect();
        listeners.forEach(remove => remove());
    }};
}
"#
)]
extern "C" {
    type NavigationDrawer;
    fn mount_navigation_drawer(
        dialog: &leptos::web_sys::HtmlElement,
        menu: &leptos::web_sys::HtmlElement,
        rail_toggle: &leptos::web_sys::HtmlElement,
        close: &leptos::web_sys::HtmlElement,
        shell: &leptos::web_sys::HtmlElement,
        decide: &leptos::wasm_bindgen::JsValue,
    ) -> NavigationDrawer;
    #[wasm_bindgen(method)]
    fn dispose(this: &NavigationDrawer);
    #[wasm_bindgen(method)]
    fn route_changed(this: &NavigationDrawer);
}
