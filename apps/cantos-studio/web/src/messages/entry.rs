//! Login-only Studio resources; independent of Theatre authentication and membership copy.
use crate::entry::Entry;
use crate::localization::generated::entry as resources;

#[derive(Clone, Copy)]
pub enum Key {
    LoginTitle,
    LoginHelp,
    Welcome,
    WelcomeHelp,
    Checking,
    Denied,
    DeniedHelp,
    Unavailable,
    UnavailableHelp,
    SignInUnavailable,
    SignInUnavailableHelp,
    Retained,
    RetainedHelp,
    Dashboard,
    Import,
    Back,
    Cancel,
    Retry,
    Session,
    CloseAccount,
    MemoryHelp,
    Reauthenticate,
    CleanupPending,
}

pub fn title(entry: Entry) -> Key {
    match entry {
        Entry::Checking => Key::Checking,
        Entry::Denied => Key::Denied,
        Entry::Unavailable => Key::Unavailable,
        Entry::SignInUnavailable => Key::SignInUnavailable,
        Entry::RetainedOwner => Key::Retained,
        Entry::Login => Key::LoginTitle,
        Entry::Workspace => Key::Dashboard,
    }
}

pub fn help(entry: Entry) -> Key {
    match entry {
        Entry::Checking | Entry::Login => Key::LoginHelp,
        Entry::Denied => Key::DeniedHelp,
        Entry::Unavailable => Key::UnavailableHelp,
        Entry::SignInUnavailable => Key::SignInUnavailableHelp,
        Entry::RetainedOwner => Key::RetainedHelp,
        Entry::Workspace => Key::WelcomeHelp,
    }
}

pub fn text(key: Key, locale: crate::localization::Locale) -> &'static str {
    match key {
        Key::CleanupPending => resources::text(resources::Key::CleanupPending, locale),
        Key::MemoryHelp => resources::text(resources::Key::MemoryHelp, locale),
        Key::SignInUnavailable => resources::text(resources::Key::SignInUnavailable, locale),
        Key::SignInUnavailableHelp => {
            resources::text(resources::Key::SignInUnavailableHelp, locale)
        }
        Key::Reauthenticate => resources::text(resources::Key::Reauthenticate, locale),
        Key::LoginTitle => resources::text(resources::Key::LoginTitle, locale),
        Key::LoginHelp => resources::text(resources::Key::LoginHelp, locale),
        Key::Welcome => resources::text(resources::Key::Welcome, locale),
        Key::WelcomeHelp => resources::text(resources::Key::WelcomeHelp, locale),
        Key::Checking => resources::text(resources::Key::Checking, locale),
        Key::Denied => resources::text(resources::Key::Denied, locale),
        Key::DeniedHelp => resources::text(resources::Key::DeniedHelp, locale),
        Key::Unavailable => resources::text(resources::Key::Unavailable, locale),
        Key::UnavailableHelp => resources::text(resources::Key::UnavailableHelp, locale),
        Key::Retained => resources::text(resources::Key::Retained, locale),
        Key::RetainedHelp => resources::text(resources::Key::RetainedHelp, locale),
        Key::Dashboard => resources::text(resources::Key::Dashboard, locale),
        Key::Import => resources::text(resources::Key::Import, locale),
        Key::Back => resources::text(resources::Key::Back, locale),
        Key::Cancel => resources::text(resources::Key::Cancel, locale),
        Key::Retry => resources::text(resources::Key::Retry, locale),
        Key::Session => resources::text(resources::Key::Session, locale),
        Key::CloseAccount => resources::text(resources::Key::CloseAccount, locale),
    }
}
