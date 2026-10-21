//! ErrorWindow observes the active notice. Queue/dismissal bookkeeping and selected actions
//! remain intact even when that notice is unchanged.
use super::{
    account_writer::{Published, Write},
    errors::ErrorCenter,
};
use leptos::prelude::*;

pub(crate) fn update_errors<R>(
    errors: RwSignal<ErrorCenter>,
    apply: impl FnOnce(&mut ErrorCenter) -> R,
) -> Write<R> {
    match errors.try_maybe_update(|center| {
        let before = center.active();
        let value = apply(center);
        let changed = before != center.active();
        (
            changed,
            (
                value,
                if changed {
                    Published::Yes
                } else {
                    Published::No
                },
            ),
        )
    }) {
        None => Write::Disposed,
        Some((value, published)) => Write::Done { value, published },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::errors::{ErrorAction, ErrorKind, ErrorNotice, ErrorOperation};
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    };

    #[test]
    fn error_writer_publishes_only_active_changes_and_returns_actions_after_release() {
        Owner::new().with(|| {
            let errors = RwSignal::new(ErrorCenter::default());
            let runs = Arc::new(AtomicUsize::new(0));
            let count = runs.clone();
            let observed = Memo::new(move |_| {
                count.fetch_add(1, Ordering::SeqCst);
                errors.with(ErrorCenter::active)
            });
            assert_eq!(observed.get(), None);
            let network = ErrorNotice::read(ErrorKind::Network, ErrorOperation::LoadCatalog);
            for _ in 0..3 {
                update_errors(errors, |c| c.report(network, false));
                observed.get();
            }
            assert_eq!(runs.load(Ordering::SeqCst), 2);
            // A queued notice is retained without waking a view of the same active notice.
            let work = ErrorNotice::read(ErrorKind::Timeout, ErrorOperation::LoadWork);
            update_errors(errors, |c| c.report(work, false));
            observed.get();
            assert_eq!(runs.load(Ordering::SeqCst), 2);
            let action = update_errors(errors, |c| c.choose(ErrorAction::Retry))
                .into_value()
                .flatten();
            // Reenter the owner after the selected effect has been returned.
            assert_eq!(errors.with_untracked(ErrorCenter::active), Some(work));
            assert_eq!(action, Some(ErrorOperation::LoadCatalog));
            observed.get();
            assert_eq!(runs.load(Ordering::SeqCst), 3);
            update_errors(errors, |c| c.resolve(ErrorOperation::LoadCatalog));
            observed.get();
            assert_eq!(runs.load(Ordering::SeqCst), 3);
            update_errors(errors, ErrorCenter::dismiss);
            assert_eq!(observed.get(), None);
            update_errors(errors, ErrorCenter::dismiss);
            assert_eq!(observed.get(), None);
            assert_eq!(
                runs.load(Ordering::SeqCst),
                4,
                "duplicate close must not notify"
            );
            update_errors(errors, |c| c.report(work, false));
            observed.get();
            assert_eq!(runs.load(Ordering::SeqCst), 4);
        });
    }
}
