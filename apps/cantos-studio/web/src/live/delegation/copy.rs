//! Delegation panel copy in both supported locales. Contract labels such as `work:read` are
//! shown beside plain-language names so an owner can match them to delegate documentation.
use super::{GrantState, Lifetime, Problem};
use crate::contract::DelegationAction;
use crate::localization::generated::delegation as resources;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Key {
    Title,
    Intro,
    MemberOnly,
    ForbiddenHelp,
    IssueTitle,
    Delegate,
    DelegateHelp,
    Actions,
    ActionsHelp,
    Lifetime,
    LifetimeHelp,
    Issue,
    SaveTokenFirst,
    Archived,
    RevealTitle,
    RevealBody,
    RevealField,
    RevealUse,
    Copy,
    Copied,
    CopyManual,
    Dismiss,
    UnrecoverableTitle,
    UnrecoverableBody,
    RevokeThis,
    ListTitle,
    Refresh,
    More,
    Empty,
    EmptyHint,
    NotLoaded,
    WorkLabel,
    DelegateLabel,
    GrantId,
    Issued,
    Expires,
    RevokedAt,
    RevokedNow,
    Revoke,
    RevokeFor,
    DialogTitle,
    DialogBody,
    DialogKeep,
    DialogConfirm,
    DialogClose,
    DialogFailed,
    Idle,
    Loading,
    Issuing,
    Revoking,
    Listed,
    IssuedOk,
    Replayed,
    RevokedOk,
    Ambiguous,
    Retry,
    Abandon,
    AbandonHelp,
    Cancel,
}

pub fn text(key: Key, locale: crate::localization::Locale) -> &'static str {
    match key {
        Key::Title => resources::text(resources::Key::Title, locale),
        Key::Intro => resources::text(resources::Key::Intro, locale),
        Key::MemberOnly => resources::text(resources::Key::MemberOnly, locale),
        Key::ForbiddenHelp => resources::text(resources::Key::ForbiddenHelp, locale),
        Key::IssueTitle => resources::text(resources::Key::IssueTitle, locale),
        Key::Delegate => resources::text(resources::Key::Delegate, locale),
        Key::DelegateHelp => resources::text(resources::Key::DelegateHelp, locale),
        Key::Actions => resources::text(resources::Key::Actions, locale),
        Key::ActionsHelp => resources::text(resources::Key::ActionsHelp, locale),
        Key::Lifetime => resources::text(resources::Key::Lifetime, locale),
        Key::LifetimeHelp => resources::text(resources::Key::LifetimeHelp, locale),
        Key::Issue => resources::text(resources::Key::Issue, locale),
        Key::SaveTokenFirst => resources::text(resources::Key::SaveTokenFirst, locale),
        Key::Archived => resources::text(resources::Key::Archived, locale),
        Key::RevealTitle => resources::text(resources::Key::RevealTitle, locale),
        Key::RevealBody => resources::text(resources::Key::RevealBody, locale),
        Key::RevealField => resources::text(resources::Key::RevealField, locale),
        Key::RevealUse => resources::text(resources::Key::RevealUse, locale),
        Key::Copy => resources::text(resources::Key::Copy, locale),
        Key::Copied => resources::text(resources::Key::Copied, locale),
        Key::CopyManual => resources::text(resources::Key::CopyManual, locale),
        Key::Dismiss => resources::text(resources::Key::Dismiss, locale),
        Key::UnrecoverableTitle => resources::text(resources::Key::UnrecoverableTitle, locale),
        Key::UnrecoverableBody => resources::text(resources::Key::UnrecoverableBody, locale),
        Key::RevokeThis => resources::text(resources::Key::RevokeThis, locale),
        Key::ListTitle => resources::text(resources::Key::ListTitle, locale),
        Key::Refresh => resources::text(resources::Key::Refresh, locale),
        Key::More => resources::text(resources::Key::More, locale),
        Key::Empty => resources::text(resources::Key::Empty, locale),
        Key::EmptyHint => resources::text(resources::Key::EmptyHint, locale),
        Key::WorkLabel => resources::text(resources::Key::WorkLabel, locale),
        Key::NotLoaded => resources::text(resources::Key::NotLoaded, locale),
        Key::DelegateLabel => resources::text(resources::Key::DelegateLabel, locale),
        Key::GrantId => resources::text(resources::Key::GrantId, locale),
        Key::Issued => resources::text(resources::Key::Issued, locale),
        Key::Expires => resources::text(resources::Key::Expires, locale),
        Key::RevokedAt => resources::text(resources::Key::RevokedAt, locale),
        Key::RevokedNow => resources::text(resources::Key::RevokedNow, locale),
        Key::Revoke => resources::text(resources::Key::Revoke, locale),
        Key::RevokeFor => resources::text(resources::Key::RevokeFor, locale),
        Key::DialogTitle => resources::text(resources::Key::DialogTitle, locale),
        Key::DialogBody => resources::text(resources::Key::DialogBody, locale),
        Key::DialogKeep => resources::text(resources::Key::DialogKeep, locale),
        Key::DialogConfirm => resources::text(resources::Key::DialogConfirm, locale),
        Key::DialogClose => resources::text(resources::Key::DialogClose, locale),
        Key::DialogFailed => resources::text(resources::Key::DialogFailed, locale),
        Key::Idle => resources::text(resources::Key::Idle, locale),
        Key::Loading => resources::text(resources::Key::Loading, locale),
        Key::Issuing => resources::text(resources::Key::Issuing, locale),
        Key::Revoking => resources::text(resources::Key::Revoking, locale),
        Key::Listed => resources::text(resources::Key::Listed, locale),
        Key::IssuedOk => resources::text(resources::Key::IssuedOk, locale),
        Key::Replayed => resources::text(resources::Key::Replayed, locale),
        Key::RevokedOk => resources::text(resources::Key::RevokedOk, locale),
        Key::Ambiguous => resources::text(resources::Key::Ambiguous, locale),
        Key::Retry => resources::text(resources::Key::Retry, locale),
        Key::Abandon => resources::text(resources::Key::Abandon, locale),
        Key::AbandonHelp => resources::text(resources::Key::AbandonHelp, locale),
        Key::Cancel => resources::text(resources::Key::Cancel, locale),
    }
}

pub fn action(action: DelegationAction, locale: crate::localization::Locale) -> &'static str {
    match action {
        DelegationAction::WorkRead => resources::text(resources::Key::ActionWorkRead, locale),
        DelegationAction::ContentRead => resources::text(resources::Key::ActionContentRead, locale),
        DelegationAction::ContentWrite => {
            resources::text(resources::Key::ActionContentWrite, locale)
        }
    }
}

/// Short name used in grant rows.
pub fn action_short(action: DelegationAction, locale: crate::localization::Locale) -> &'static str {
    match action {
        DelegationAction::WorkRead => resources::text(resources::Key::ActionShortWorkRead, locale),
        DelegationAction::ContentRead => {
            resources::text(resources::Key::ActionShortContentRead, locale)
        }
        DelegationAction::ContentWrite => {
            resources::text(resources::Key::ActionShortContentWrite, locale)
        }
    }
}

/// The contract label, never translated.
pub const fn action_label(action: DelegationAction) -> &'static str {
    match action {
        DelegationAction::WorkRead => "work:read",
        DelegationAction::ContentRead => "content:read",
        DelegationAction::ContentWrite => "content:write",
    }
}

pub fn lifetime(lifetime: Lifetime, locale: crate::localization::Locale) -> &'static str {
    match lifetime {
        Lifetime::OneDay => resources::text(resources::Key::LifetimeOneDay, locale),
        Lifetime::Week => resources::text(resources::Key::LifetimeWeek, locale),
        Lifetime::TwoWeeks => resources::text(resources::Key::LifetimeTwoWeeks, locale),
        Lifetime::Maximum => resources::text(resources::Key::LifetimeMaximum, locale),
    }
}

pub fn state(state: GrantState, locale: crate::localization::Locale) -> &'static str {
    match state {
        GrantState::Active => resources::text(resources::Key::StateActive, locale),
        GrantState::Expired => resources::text(resources::Key::StateExpired, locale),
        GrantState::Revoked => resources::text(resources::Key::StateRevoked, locale),
    }
}

pub fn problem(problem: &Problem, locale: crate::localization::Locale) -> String {
    match problem {
        Problem::InvalidActor => {
            resources::text(resources::Key::ProblemInvalidActor, locale).to_owned()
        }
        Problem::SelfDelegation => {
            resources::text(resources::Key::ProblemSelfDelegation, locale).to_owned()
        }
        Problem::UnavailableActor => {
            resources::text(resources::Key::ProblemUnavailableActor, locale).to_owned()
        }
        Problem::EmptyActions => {
            resources::text(resources::Key::ProblemEmptyActions, locale).to_owned()
        }
        Problem::DuplicateAction => {
            resources::text(resources::Key::ProblemDuplicateAction, locale).to_owned()
        }
        Problem::InvalidExpiry => {
            resources::text(resources::Key::ProblemInvalidExpiry, locale).to_owned()
        }
        Problem::DuplicateGrantId => {
            resources::text(resources::Key::ProblemDuplicateGrantId, locale).to_owned()
        }
        Problem::Archived => resources::text(resources::Key::ProblemArchived, locale).to_owned(),
        Problem::OperationReused => {
            resources::text(resources::Key::ProblemOperationReused, locale).to_owned()
        }
        Problem::NotOwner => resources::text(resources::Key::ProblemNotOwner, locale).to_owned(),
        Problem::NotFound => resources::text(resources::Key::ProblemNotFound, locale).to_owned(),
        Problem::SignIn => resources::text(resources::Key::ProblemSignIn, locale).to_owned(),
        Problem::Malformed => resources::text(resources::Key::ProblemMalformed, locale).to_owned(),
        Problem::Unavailable => {
            resources::text(resources::Key::ProblemUnavailable, locale).to_owned()
        }
        Problem::Interrupted => {
            resources::text(resources::Key::ProblemInterrupted, locale).to_owned()
        }
        Problem::UnexpectedReply => {
            resources::text(resources::Key::ProblemUnexpectedReply, locale).to_owned()
        }
        Problem::InvalidInput => {
            resources::text(resources::Key::ProblemInvalidInput, locale).to_owned()
        }
        Problem::Unrecognized { path, rule } => crate::localization::present(
            crate::localization::generated::Message::StudioDelegationProblemUnrecognized {
                arg_path: path,
                arg_rule: rule,
            }
            .render(locale),
            locale,
        ),
    }
}
