//! Changed paths relative to a base ref: committed since the merge base, staged, unstaged and
//! untracked. Renames are split (`--no-renames`) so both the old and new path count.

use std::{collections::BTreeSet, path::Path, process::Command};

use anyhow::{bail, Context, Result};

pub fn changed_paths(root: &Path, base: &str) -> Result<Vec<String>> {
    let verify = git(
        root,
        &[
            "rev-parse",
            "--verify",
            "--quiet",
            &format!("{base}^{{commit}}"),
        ],
    );
    if verify.is_err() {
        bail!(
            "base ref `{base}` does not exist locally; run `git fetch origin develop` or pass --base <ref>"
        );
    }
    let comparison = format!("{base}...HEAD");
    let mut paths = BTreeSet::new();
    for args in [
        &[
            "diff",
            "--name-only",
            "--no-renames",
            "-z",
            &comparison,
            "--",
        ][..],
        &["diff", "--name-only", "--no-renames", "-z", "--"],
        &[
            "diff",
            "--cached",
            "--name-only",
            "--no-renames",
            "-z",
            "--",
        ],
        &["ls-files", "--others", "--exclude-standard", "-z"],
    ] {
        paths.extend(split_nul(&git(root, args)?)?);
    }
    Ok(paths.into_iter().collect())
}

fn git(root: &Path, args: &[&str]) -> Result<Vec<u8>> {
    let output = Command::new("git")
        .args(args)
        .current_dir(root)
        .output()
        .with_context(|| format!("could not run git {}", args.join(" ")))?;
    if !output.status.success() {
        bail!(
            "git {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    Ok(output.stdout)
}

fn split_nul(bytes: &[u8]) -> Result<Vec<String>> {
    bytes
        .split(|byte| *byte == 0)
        .filter(|path| !path.is_empty())
        .map(|path| {
            std::str::from_utf8(path)
                .map(str::to_owned)
                .context("git returned a non-UTF-8 path")
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{fs, process::Command};

    fn git_in(dir: &Path, args: &[&str]) {
        let status = Command::new("git")
            .args([
                "-c",
                "user.name=xtask",
                "-c",
                "user.email=xtask@example.invalid",
            ])
            .args(args)
            .current_dir(dir)
            .output()
            .unwrap();
        assert!(status.status.success(), "git {args:?}: {status:?}");
    }

    #[test]
    fn collects_committed_staged_unstaged_untracked_and_both_rename_ends() {
        let temp = std::env::temp_dir().join(format!("xtask-changes-{}", std::process::id()));
        let _ = fs::remove_dir_all(&temp);
        fs::create_dir_all(&temp).unwrap();
        git_in(&temp, &["init", "-q", "-b", "develop"]);
        for name in ["kept.md", "renamed.md", "edited.md", "staged.md"] {
            fs::write(temp.join(name), name).unwrap();
        }
        git_in(&temp, &["add", "."]);
        git_in(&temp, &["commit", "-q", "-m", "base"]);
        git_in(&temp, &["branch", "base"]);
        git_in(&temp, &["mv", "renamed.md", "moved.md"]);
        git_in(&temp, &["commit", "-q", "-m", "rename"]);
        fs::write(temp.join("edited.md"), "changed").unwrap();
        fs::write(temp.join("staged.md"), "changed").unwrap();
        git_in(&temp, &["add", "staged.md"]);
        fs::write(temp.join("new.md"), "new").unwrap();

        let paths = changed_paths(&temp, "base").unwrap();
        assert_eq!(
            paths,
            ["edited.md", "moved.md", "new.md", "renamed.md", "staged.md"]
        );
        let error = changed_paths(&temp, "no-such-base").unwrap_err();
        assert!(error.to_string().contains("does not exist"), "{error}");
        fs::remove_dir_all(&temp).unwrap();
    }
}
