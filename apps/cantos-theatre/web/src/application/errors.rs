//! Pure counterpart of CMP ErrorNotice/ErrorCenter. No transport, renderer or retry engine.

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum ErrorKind {
    Network,
    Timeout,
    Unauthenticated,
    Forbidden,
    ContentUnavailable,
    Protocol,
    Configuration,
    Storage,
    ServiceUnavailable,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum ErrorOperation {
    LoadCatalog,
    LoadWork,
    LoadUnits,
    StartPlayback,
    UpdateLibrary,
    SignIn,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ErrorScope {
    Host,
    Section,
    Action,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ErrorAction {
    Retry,
    SignIn,
    Close,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct ErrorKey {
    pub kind: ErrorKind,
    pub operation: ErrorOperation,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ErrorNotice {
    pub key: ErrorKey,
    retryable: bool,
}
impl ErrorNotice {
    pub fn read(kind: ErrorKind, operation: ErrorOperation) -> Self {
        Self {
            key: ErrorKey { kind, operation },
            retryable: matches!(
                operation,
                ErrorOperation::LoadCatalog | ErrorOperation::LoadWork | ErrorOperation::LoadUnits
            ),
        }
    }
    pub fn scope(self) -> ErrorScope {
        match (self.key.kind, self.key.operation) {
            (ErrorKind::Configuration, _) => ErrorScope::Host,
            (
                _,
                ErrorOperation::LoadCatalog | ErrorOperation::LoadWork | ErrorOperation::LoadUnits,
            ) => ErrorScope::Section,
            _ => ErrorScope::Action,
        }
    }
    pub fn primary(self) -> ErrorAction {
        match self.key.kind {
            ErrorKind::Unauthenticated => ErrorAction::SignIn,
            ErrorKind::Network
            | ErrorKind::Timeout
            | ErrorKind::Protocol
            | ErrorKind::ServiceUnavailable
                if self.retryable =>
            {
                ErrorAction::Retry
            }
            // Configuration/storage need correction; mutation replay belongs to its
            // receipt owner. Never infer a safe write retry from a transport error.
            _ => ErrorAction::Close,
        }
    }
    pub fn modal(self, has_content: bool) -> bool {
        match self.key.kind {
            ErrorKind::Configuration | ErrorKind::Unauthenticated | ErrorKind::Forbidden => true,
            ErrorKind::ContentUnavailable => false,
            _ => !has_content,
        }
    }
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ErrorCenter {
    active: Option<ErrorNotice>,
    queued: Vec<ErrorNotice>,
    dismissed: std::collections::BTreeSet<ErrorKey>,
}
impl ErrorCenter {
    pub fn active(&self) -> Option<ErrorNotice> {
        self.active
    }
    pub fn report(&mut self, notice: ErrorNotice, has_content: bool) {
        if !notice.modal(has_content)
            || self.dismissed.contains(&notice.key)
            || self.active.is_some_and(|n| n.key == notice.key)
            || self.queued.iter().any(|n| n.key == notice.key)
        {
            return;
        }
        if self.active.is_none() {
            self.active = Some(notice);
        } else if self.queued.len() < 3 {
            self.queued.push(notice);
        }
    }
    pub fn resolve(&mut self, operation: ErrorOperation) {
        self.queued.retain(|n| n.key.operation != operation);
        self.dismissed.retain(|k| k.operation != operation);
        if self.active.is_some_and(|n| n.key.operation == operation) {
            self.promote();
        }
    }
    pub fn dismiss(&mut self) {
        if let Some(notice) = self.active {
            self.dismissed.insert(notice.key);
            self.promote();
        }
    }
    pub fn choose(&mut self, action: ErrorAction) -> Option<ErrorOperation> {
        let notice = self.active?;
        if action == ErrorAction::Close {
            self.dismiss();
            return None;
        }
        if action != notice.primary() {
            return None;
        }
        self.promote();
        Some(notice.key.operation)
    }
    fn promote(&mut self) {
        self.active = if self.queued.is_empty() {
            None
        } else {
            Some(self.queued.remove(0))
        };
    }
}
impl From<super::public_work::ReadFailure> for ErrorKind {
    fn from(value: super::public_work::ReadFailure) -> Self {
        match value {
            super::public_work::ReadFailure::Diagnostic(kind) => kind,
            super::public_work::ReadFailure::Unavailable => Self::ServiceUnavailable,
            super::public_work::ReadFailure::Protocol => Self::Protocol,
        }
    }
}
impl From<super::public_discovery::PublicDiscoveryFailure> for ErrorKind {
    fn from(value: super::public_discovery::PublicDiscoveryFailure) -> Self {
        match value {
            super::public_discovery::PublicDiscoveryFailure::Diagnostic(kind) => kind,
            super::public_discovery::PublicDiscoveryFailure::Unavailable => {
                Self::ServiceUnavailable
            }
            super::public_discovery::PublicDiscoveryFailure::Protocol => Self::Protocol,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn concurrent_failures_are_bounded_dismissed_episodes_stay_closed_until_resolution() {
        let mut center = ErrorCenter::default();
        let network = ErrorNotice::read(ErrorKind::Network, ErrorOperation::LoadCatalog);
        for _ in 0..20 {
            center.report(network, false);
        }
        for kind in [
            ErrorKind::Timeout,
            ErrorKind::Forbidden,
            ErrorKind::Protocol,
            ErrorKind::Storage,
        ] {
            center.report(ErrorNotice::read(kind, ErrorOperation::LoadCatalog), false);
        }
        assert_eq!(center.active(), Some(network));
        assert_eq!(center.queued.len(), 3);
        center.dismiss();
        center.report(network, false);
        assert_eq!(center.queued.len(), 2);
        center.resolve(ErrorOperation::LoadCatalog);
        assert_eq!(center, ErrorCenter::default());
        center.report(network, false);
        assert_eq!(
            center.choose(ErrorAction::Retry),
            Some(ErrorOperation::LoadCatalog)
        );
        center.report(network, false);
        assert_eq!(center.active(), Some(network));
    }
    #[test]
    fn stale_rows_remain_inline_and_mutations_never_offer_replay() {
        let mut center = ErrorCenter::default();
        center.report(
            ErrorNotice::read(ErrorKind::Network, ErrorOperation::LoadCatalog),
            true,
        );
        assert_eq!(center.active(), None);
        for kind in [
            ErrorKind::Network,
            ErrorKind::Timeout,
            ErrorKind::Storage,
            ErrorKind::Protocol,
        ] {
            let notice = ErrorNotice::read(kind, ErrorOperation::UpdateLibrary);
            assert_eq!(notice.primary(), ErrorAction::Close);
            center.report(notice, false);
            let before = center.clone();
            assert_eq!(center.choose(ErrorAction::Retry), None);
            assert_eq!(center, before);
            center.dismiss();
        }
    }
}
