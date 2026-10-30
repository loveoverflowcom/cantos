//! Browser effects for the Theatre-owned adaptive navigation composition.
use leptos::{html, prelude::*};

pub(crate) fn bind(
    dialog: NodeRef<html::Dialog>,
    menu: NodeRef<html::Button>,
    close: NodeRef<html::Button>,
    route: ReadSignal<crate::app::routes::Route>,
) {
    #[cfg(target_arch = "wasm32")]
    {
        let binding = StoredValue::new_local(None::<NavigationDrawer>);
        Effect::new(move |_| {
            let (Some(dialog), Some(menu), Some(close)) = (dialog.get(), menu.get(), close.get())
            else {
                return;
            };
            binding.update_value(|current| {
                if let Some(previous) = current.take() {
                    previous.dispose();
                }
                *current = Some(mount_navigation_drawer(&dialog, &menu, &close));
            });
        });
        Effect::new(move |_| {
            route.track();
            binding.with_value(|current| {
                if let Some(current) = current {
                    current.route_changed();
                }
            });
        });
        on_cleanup(move || {
            binding.try_update_value(|current| {
                if let Some(current) = current.take() {
                    current.dispose();
                }
            });
        });
    }
    #[cfg(not(target_arch = "wasm32"))]
    let _ = (dialog, menu, close, route);
}

#[cfg(target_arch = "wasm32")]
#[leptos::wasm_bindgen::prelude::wasm_bindgen(
    wasm_bindgen = leptos::wasm_bindgen,
    inline_js = r#"
export function mount_navigation_drawer(dialog, menu, closeButton) {
    const doc = dialog.ownerDocument;
    const win = doc.defaultView;
    const wide = win.matchMedia('(min-width: 840px)');
    const listeners = [];
    let overflow = null;
    let disposed = false;
    const listen = (target, type, callback) => {
        target.addEventListener(type, callback);
        listeners.push(() => target.removeEventListener(type, callback));
    };
    const unlock = () => {
        if (overflow !== null) {
            if (overflow.value) doc.body.style.setProperty('overflow', overflow.value, overflow.priority);
            else doc.body.style.removeProperty('overflow');
            overflow = null;
        }
        menu.setAttribute('aria-expanded', 'false');
    };
    const close = (returnFocus = true) => {
        if (!dialog.open) { unlock(); return; }
        dialog.close();
        unlock();
        if (returnFocus && !wide.matches && menu.isConnected) menu.focus({preventScroll: true});
    };
    listen(menu, 'click', () => {
        if (disposed || wide.matches || dialog.open) return;
        menu.focus({preventScroll: true});
        try { dialog.showModal(); } catch { unlock(); return; }
        overflow = {value: doc.body.style.getPropertyValue('overflow'), priority: doc.body.style.getPropertyPriority('overflow')};
        doc.body.style.setProperty('overflow', 'hidden');
        menu.setAttribute('aria-expanded', 'true');
        (dialog.querySelector('[aria-current="page"]') || closeButton).focus({preventScroll: true});
    });
    listen(closeButton, 'click', () => close());
    listen(dialog, 'cancel', event => { event.preventDefault(); close(); });
    listen(dialog, 'close', () => { if (!dialog.open) unlock(); });
    listen(dialog, 'click', event => {
        if (event.target.closest('a[href]')) { close(false); return; }
        if (event.target !== dialog) return;
        const rect = dialog.getBoundingClientRect();
        if (event.clientX < rect.left || event.clientX > rect.right || event.clientY < rect.top || event.clientY > rect.bottom) close();
    });
    // Native modal inertness contains focus; explicitly cycle Tab so browser chrome cannot
    // receive it while the drawer is open, including Shift+Tab at the first destination.
    listen(dialog, 'keydown', event => {
        if (event.key !== 'Tab') return;
        const targets = [...dialog.querySelectorAll('button:not([disabled]), a[href]')].filter(item => item.getClientRects().length);
        const first = targets[0], last = targets[targets.length - 1];
        if (event.shiftKey && doc.activeElement === first) { event.preventDefault(); last.focus(); }
        else if (!event.shiftKey && doc.activeElement === last) { event.preventDefault(); first.focus(); }
    });
    const routeChanged = () => close(false);
    listen(win, 'hashchange', routeChanged);
    listen(win, 'popstate', routeChanged);
    listen(wide, 'change', () => {
        if (!wide.matches || !dialog.open) return;
        close(false);
        const target = doc.querySelector('.theatre-rail [aria-current="page"]') || doc.getElementById('theatre-main');
        target?.focus({preventScroll: true});
    });
    return {route_changed() {
        if (!dialog.open) return;
        close(false);
        doc.getElementById('theatre-main')?.focus({preventScroll: true});
    }, dispose() {
        disposed = true;
        close(false);
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
        close: &leptos::web_sys::HtmlElement,
    ) -> NavigationDrawer;
    #[wasm_bindgen(method)]
    fn dispose(this: &NavigationDrawer);
    #[wasm_bindgen(method)]
    fn route_changed(this: &NavigationDrawer);
}
