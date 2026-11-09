//! Editing a registered repo: rename, base branch, relocate or move its folder.
//! `plan` is the pure part (what was asked, checked against the registry); `apply_disk` does
//! the git and disk work. Both are blocking where they touch git: call from `spawn_blocking`.

use std::path::{Path, PathBuf};

use super::names;
use crate::cmd::{probe_blocking, run_blocking};
use crate::model::{PatchRepo, Repo};
use crate::{Error, Result};

/// A checked edit request. Nothing in it has touched git or the disk yet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditPlan {
    pub name: String,
    pub base_branch: Option<String>,
    /// Absolute, `~` expanded, not yet checked against the disk.
    pub path: Option<PathBuf>,
    pub move_folder: bool,
}

fn blank(what: &str) -> Error {
    Error::InvalidInput(format!("{what} must not be blank"))
}

/// Checks `patch` against `current` and the other `known` repos: something must change, a new
/// name is valid and free (ignoring case, but `current` itself may change its case), a base
/// branch is not blank, a path is absolute, and `moveFolder` comes with a path.
pub fn plan(known: &[Repo], current: &Repo, patch: &PatchRepo, home: Option<&Path>) -> Result<EditPlan> {
    if patch.name.is_none() && patch.base_branch.is_none() && patch.path.is_none() {
        return Err(Error::InvalidInput("nothing to change".into()));
    }
    if patch.move_folder && patch.path.is_none() {
        return Err(Error::InvalidInput(
            "moveFolder needs the path to move the folder to".into(),
        ));
    }
    let name = match patch.name.as_deref().map(str::trim) {
        None => current.name.clone(),
        Some("") => return Err(blank("repo name")),
        Some(given) => {
            names::validate(given)?;
            let clash = known
                .iter()
                .any(|r| names::same(&r.name, given) && !names::same(&r.name, &current.name));
            if clash {
                return Err(Error::Conflict(format!(
                    "a repo named `{given}` already exists"
                )));
            }
            given.to_string()
        }
    };
    let base_branch = match patch.base_branch.as_deref().map(str::trim) {
        None => None,
        Some("") => return Err(blank("base branch")),
        Some(given) => Some(given.to_string()),
    };
    let path = match patch.path.as_deref().map(str::trim) {
        None => None,
        Some("") => return Err(blank("path")),
        Some(given) => Some(absolute(given, home)?),
    };
    Ok(EditPlan {
        name,
        base_branch,
        path,
        move_folder: patch.move_folder,
    })
}

fn absolute(given: &str, home: Option<&Path>) -> Result<PathBuf> {
    let expanded = match (given.strip_prefix('~'), home) {
        (Some(""), Some(home)) => home.to_path_buf(),
        (Some(rest), Some(home)) if rest.starts_with('/') => {
            home.join(rest.trim_start_matches('/'))
        }
        _ => PathBuf::from(given),
    };
    if expanded.is_absolute() {
        Ok(expanded)
    } else {
        Err(Error::InvalidInput(format!(
            "path must be absolute: {given}"
        )))
    }
}

/// The root commits of `path`: what two folders of one git repo share.
fn root_commits(path: &Path) -> Result<Vec<String>> {
    let out = probe_blocking("git", &["rev-list", "--max-parents=0", "HEAD"], Some(path))?;
    let mut roots: Vec<String> = out.lines().map(str::to_string).collect();
    roots.sort();
    Ok(roots)
}

/// Ok when `new` is the same git repo as `repo.path`: they share their root commits. When the
/// old folder is gone from the disk (moved by hand) there is nothing to compare with; the
/// registered base branch must then at least exist in `new`.
pub fn check_same_repo(repo: &Repo, new: &Path) -> Result<()> {
    if repo.path.exists() {
        let (a, b) = (root_commits(&repo.path)?, root_commits(new)?);
        if !a.is_empty() && a == b {
            return Ok(());
        }
        return Err(Error::InvalidInput(format!(
            "{} is not the same git repo as {}",
            new.display(),
            repo.path.display()
        )));
    }
    super::verify_branch(new, &repo.base_branch).map_err(|_| {
        Error::InvalidInput(format!(
            "{} is gone and {} has no branch `{}`: it does not look like repo `{}`",
            repo.path.display(),
            new.display(),
            repo.base_branch,
            repo.name
        ))
    })
}

/// What the disk and git say after an edit: the canonical folder and the base branch to store.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Applied {
    pub path: PathBuf,
    pub base_branch: String,
}

/// Does the git and disk part of `plan`. Every check runs before anything moves, so a refusal
/// leaves the folder where it was. `others` are the other registered repos.
pub fn apply_disk(home: &crate::Home, repo: &Repo, plan: &EditPlan, others: &[Repo]) -> Result<Applied> {
    let worktrees = home.root().join(crate::home::WORKTREES_DIR);
    let wanted = match &plan.path {
        None => None,
        Some(p) if plan.move_folder => Some(p.clone()),
        Some(p) => Some(super::resolve_path(&p.to_string_lossy())?),
    };
    if let Some(path) = &wanted {
        let taken = others.iter().find(|r| &r.path == path);
        if let Some(other) = taken {
            return Err(Error::Conflict(format!(
                "{} is already registered as `{}`",
                path.display(),
                other.name
            )));
        }
    }
    let relocated = wanted.as_ref().filter(|p| **p != repo.path);
    match (relocated, plan.move_folder) {
        (Some(target), true) => {
            check_move(repo, target)?;
            if let Some(branch) = &plan.base_branch {
                super::verify_branch(&repo.path, branch)?;
            }
            check_no_worktrees(repo)?;
            std::fs::rename(&repo.path, target).map_err(|e| {
                Error::io(
                    format!("move {} to {}", repo.path.display(), target.display()),
                    e,
                )
            })?;
        }
        (Some(target), false) => {
            super::check_top_folder(target, &worktrees)?;
            check_same_repo(repo, target)?;
            if repo.path.exists() {
                check_no_worktrees(repo)?;
            }
        }
        (None, _) => {}
    }
    let path = match relocated {
        Some(target) => target
            .canonicalize()
            .map_err(|e| Error::io(format!("moved repo {}", target.display()), e))?,
        None => repo.path.clone(),
    };
    let base_branch = match &plan.base_branch {
        Some(branch) => {
            super::verify_branch(&path, branch)?;
            branch.clone()
        }
        None => repo.base_branch.clone(),
    };
    Ok(Applied { path, base_branch })
}

/// A folder move needs the old folder, a free target and an existing parent to put it in.
fn check_move(repo: &Repo, target: &Path) -> Result<()> {
    if !repo.path.exists() {
        return Err(Error::InvalidInput(format!(
            "{} is gone: point the repo at its new path instead of moving it",
            repo.path.display()
        )));
    }
    if target.exists() {
        return Err(Error::Conflict(format!(
            "{} already exists: pick a folder that does not exist yet",
            target.display()
        )));
    }
    if target.starts_with(&repo.path) {
        return Err(Error::InvalidInput(
            "a repo cannot be moved into itself".into(),
        ));
    }
    match target.parent() {
        Some(parent) if parent.is_dir() => Ok(()),
        _ => Err(Error::InvalidInput(format!(
            "the folder above {} does not exist",
            target.display()
        ))),
    }
}

/// Refuses while task worktrees hang off the repo: their folders are keyed by the repo path.
fn check_no_worktrees(repo: &Repo) -> Result<()> {
    let out = run_blocking("git", &["worktree", "list", "--porcelain"], Some(&repo.path))?;
    let linked = out.lines().filter(|l| l.starts_with("worktree ")).count() > 1;
    if linked {
        return Err(Error::Conflict(format!(
            "repo `{}` still has worktrees: finish or remove its tasks before moving it",
            repo.name
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Home;

    fn repo(name: &str, path: &str) -> Repo {
        Repo {
            name: name.into(),
            path: path.into(),
            base_branch: "main".into(),
            github_account: None,
            gitlab_account: None,
        }
    }

    fn patch() -> PatchRepo {
        PatchRepo::default()
    }

    fn planned(patch: PatchRepo) -> Result<EditPlan> {
        let known = [repo("shop", "/x/shop"), repo("cart", "/x/cart")];
        plan(&known, &known[0], &patch, Some(Path::new("/home/me")))
    }

    #[test]
    fn an_empty_edit_or_a_move_without_a_path_is_refused() {
        let err = planned(patch()).unwrap_err();
        assert!(err.to_string().contains("nothing to change"), "{err}");
        let lone_move = PatchRepo {
            move_folder: true,
            name: Some("x".into()),
            ..patch()
        };
        assert!(matches!(planned(lone_move), Err(Error::InvalidInput(_))));
    }

    #[test]
    fn a_new_name_is_valid_and_free_ignoring_case() {
        let rename = |name: &str| PatchRepo {
            name: Some(name.into()),
            ..patch()
        };
        assert_eq!(planned(rename(" store ")).unwrap().name, "store");
        assert_eq!(
            planned(rename("SHOP")).unwrap().name,
            "SHOP",
            "its own name may change case"
        );
        assert!(matches!(planned(rename("Cart")), Err(Error::Conflict(_))));
        for bad in ["", "  ", "a b", "-x", "a/b"] {
            assert!(
                matches!(planned(rename(bad)), Err(Error::InvalidInput(_))),
                "{bad:?}"
            );
        }
    }

    #[test]
    fn a_blank_branch_or_path_is_refused_and_a_path_must_be_absolute() {
        let branch = PatchRepo {
            base_branch: Some(" ".into()),
            ..patch()
        };
        assert!(matches!(planned(branch), Err(Error::InvalidInput(_))));
        let blank = PatchRepo {
            path: Some("".into()),
            ..patch()
        };
        assert!(matches!(planned(blank), Err(Error::InvalidInput(_))));
        let relative = PatchRepo {
            path: Some("work/shop".into()),
            ..patch()
        };
        assert!(matches!(planned(relative), Err(Error::InvalidInput(_))));
        let tilde = PatchRepo {
            path: Some("~/dev/shop".into()),
            move_folder: true,
            ..patch()
        };
        let got = planned(tilde).unwrap();
        assert_eq!(got.path, Some(PathBuf::from("/home/me/dev/shop")));
        assert!(got.move_folder);
        assert_eq!(got.name, "shop", "a missing name keeps the current one");
    }

    fn git(dir: &Path, args: &[&str]) {
        let id = [
            "-c",
            "user.name=t",
            "-c",
            "user.email=t@t",
            "-c",
            "commit.gpgsign=false",
        ];
        let ok = std::process::Command::new("git")
            .args(id)
            .args(args)
            .current_dir(dir)
            .status()
            .unwrap()
            .success();
        assert!(ok, "git {args:?}");
    }

    /// A repo with one commit (`seed` is its message, so two seeds are two histories) on
    /// `main` and a second branch `dev`.
    fn init(dir: &Path, seed: &str) -> Repo {
        std::fs::create_dir_all(dir).unwrap();
        git(dir, &["init", "-q", "-b", "main"]);
        git(dir, &["commit", "-q", "--allow-empty", "-m", seed]);
        git(dir, &["branch", "dev"]);
        let path = dir.canonicalize().unwrap();
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        repo(&name, &path.to_string_lossy())
    }

    struct World {
        tmp: tempfile::TempDir,
        home: Home,
    }

    fn world() -> World {
        let tmp = tempfile::tempdir().unwrap();
        let home = Home::at(tmp.path().join(".townhall"));
        World { tmp, home }
    }

    fn apply(w: &World, repo: &Repo, patch: PatchRepo, others: &[Repo]) -> Result<Applied> {
        let known: Vec<Repo> = std::iter::once(repo.clone())
            .chain(others.iter().cloned())
            .collect();
        let p = plan(&known, repo, &patch, None)?;
        apply_disk(&w.home, repo, &p, others)
    }

    fn to(p: &Path) -> PatchRepo {
        PatchRepo {
            path: Some(p.to_string_lossy().into_owned()),
            ..patch()
        }
    }

    #[test]
    fn the_base_branch_must_exist() {
        let w = world();
        let shop = init(&w.tmp.path().join("shop"), "one");
        let ok = PatchRepo {
            base_branch: Some("dev".into()),
            ..patch()
        };
        assert_eq!(apply(&w, &shop, ok, &[]).unwrap().base_branch, "dev");
        let bad = PatchRepo {
            base_branch: Some("nope".into()),
            ..patch()
        };
        let err = apply(&w, &shop, bad, &[]).unwrap_err();
        assert!(err.to_string().contains("not found"), "{err}");
    }

    #[test]
    fn a_relocated_path_must_be_the_same_git_repo() {
        let w = world();
        let shop = init(&w.tmp.path().join("shop"), "one");
        let copy = w.tmp.path().join("copy");
        let (from, into) = (shop.path.to_string_lossy(), copy.to_string_lossy());
        git(w.tmp.path(), &["clone", "-q", &from, &into]);
        let got = apply(&w, &shop, to(&copy), &[]).unwrap();
        assert_eq!(got.path, copy.canonicalize().unwrap());
        let stranger = init(&w.tmp.path().join("stranger"), "other history");
        let err = apply(&w, &shop, to(&stranger.path), &[]).unwrap_err();
        assert!(err.to_string().contains("not the same git repo"), "{err}");
        let err = apply(&w, &shop, to(&w.tmp.path().join("missing")), &[]).unwrap_err();
        assert!(matches!(err, Error::InvalidInput(_)), "{err}");
    }

    #[test]
    fn a_path_another_repo_already_uses_is_a_conflict() {
        let w = world();
        let shop = init(&w.tmp.path().join("shop"), "one");
        let cart = init(&w.tmp.path().join("cart"), "one");
        let err = apply(&w, &shop, to(&cart.path), &[cart]).unwrap_err();
        assert!(matches!(err, Error::Conflict(_)), "{err}");
    }

    #[test]
    fn a_gone_folder_can_be_pointed_at_where_it_went() {
        let w = world();
        let shop = init(&w.tmp.path().join("shop"), "one");
        let new = w.tmp.path().join("moved");
        std::fs::rename(&shop.path, &new).unwrap();
        let got = apply(&w, &shop, to(&new), &[]).unwrap();
        assert_eq!(got.path, new.canonicalize().unwrap());
        let other = init(&w.tmp.path().join("other"), "two");
        git(&other.path, &["branch", "-m", "main", "trunk"]);
        assert!(
            apply(&w, &shop, to(&other.path), &[]).is_err(),
            "no `main` there"
        );
    }

    #[test]
    fn the_folder_is_moved_only_to_a_free_place() {
        let w = world();
        let shop = init(&w.tmp.path().join("shop"), "one");
        let target = w.tmp.path().join("dev/shop-renamed");
        let mv = |p: &Path| PatchRepo {
            move_folder: true,
            ..to(p)
        };
        let err = apply(&w, &shop, mv(&target), &[]).unwrap_err();
        assert!(
            err.to_string().contains("does not exist"),
            "no parent folder: {err}"
        );
        std::fs::create_dir_all(&target).unwrap();
        let err = apply(&w, &shop, mv(&target), &[]).unwrap_err();
        assert!(matches!(err, Error::Conflict(_)), "{err}");
        assert!(
            shop.path.join(".git").exists(),
            "refusals leave the folder alone"
        );
        std::fs::remove_dir_all(&target).unwrap();
        let bad_branch = PatchRepo {
            base_branch: Some("nope".into()),
            ..mv(&target)
        };
        assert!(apply(&w, &shop, bad_branch, &[]).is_err());
        assert!(
            shop.path.join(".git").exists(),
            "a bad branch is found before moving"
        );
        let got = apply(&w, &shop, mv(&target), &[]).unwrap();
        assert_eq!(got.path, target.canonicalize().unwrap());
        assert!(!shop.path.exists());
        assert!(got.path.join(".git").exists());
    }

    #[test]
    fn a_repo_with_worktrees_is_not_moved() {
        let w = world();
        let shop = init(&w.tmp.path().join("shop"), "one");
        let tree = w.tmp.path().join("tree");
        let tree_arg = tree.to_string_lossy();
        git(&shop.path, &["worktree", "add", "-q", "-b", "task", &tree_arg]);
        let target = w.tmp.path().join("elsewhere");
        let mv = PatchRepo {
            move_folder: true,
            ..to(&target)
        };
        let err = apply(&w, &shop, mv, &[]).unwrap_err();
        assert!(matches!(err, Error::Conflict(_)), "{err}");
        assert!(shop.path.exists() && !target.exists());
    }
}
