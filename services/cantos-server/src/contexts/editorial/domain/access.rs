//! The per-script access table. Every context that acts on a script asks it the same question.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Access {
    Owner,
    Editor,
    Reader,
    None,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Read,
    Write,
    Review,
}

impl Access {
    /// A stored membership role; `None` for a role this table does not know.
    pub fn from_member_role(role: &str) -> Option<Access> {
        match role {
            "reader" => Some(Access::Reader),
            "editor" => Some(Access::Editor),
            _ => None,
        }
    }
}

pub fn permits(access: Access, action: Action) -> bool {
    match (access, action) {
        (Access::Owner, _)
        | (Access::Editor, Action::Read | Action::Write)
        | (Access::Reader, Action::Read) => true,
        (Access::Editor | Access::Reader, Action::Review)
        | (Access::Reader, Action::Write)
        | (Access::None, _) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::{permits, Access, Action};

    #[test]
    fn permission_table_is_closed_and_fail_closed() {
        for (access, read, write) in [
            (Access::Owner, true, true),
            (Access::Editor, true, true),
            (Access::Reader, true, false),
            (Access::None, false, false),
        ] {
            assert_eq!(permits(access, Action::Read), read);
            assert_eq!(permits(access, Action::Write), write);
            assert_eq!(permits(access, Action::Review), access == Access::Owner);
        }
    }

    #[test]
    fn only_known_member_roles_grant_access() {
        assert_eq!(Access::from_member_role("reader"), Some(Access::Reader));
        assert_eq!(Access::from_member_role("editor"), Some(Access::Editor));
        for role in ["owner", "Reader", "", "none"] {
            assert_eq!(Access::from_member_role(role), None, "{role}");
        }
    }
}
