use std::path::{Component, Path, PathBuf};

const DATA_PHRASES: [&str; 20] = [
    "drop database",
    "drop table",
    "drop schema",
    "truncate table",
    "delete from",
    "dropdb",
    "dropuser",
    "dropdatabase",
    "db:drop",
    "db:reset",
    "db:wipe",
    "migrate:fresh",
    "migrate:reset",
    "flushall",
    "flushdb",
    "docker volume rm",
    "docker volume prune",
    "docker system prune",
    "database drop",
    "database reset",
];

const SECRET_PHRASES: [&str; 6] = [
    "security find-",
    "security add-",
    "security dump-",
    "gh auth token",
    "git credential",
    "printenv",
];

const SECRET_NAMES: [&str; 14] = [
    ".ssh",
    ".aws",
    ".gnupg",
    ".kube",
    ".docker",
    ".netrc",
    ".npmrc",
    ".pypirc",
    ".git-credentials",
    ".env",
    "id_rsa",
    "id_ed25519",
    "id_ecdsa",
    "login.keychain-db",
];

const SECRET_SUFFIXES: [&str; 5] = [".pem", ".key", ".p12", ".keychain", ".keychain-db"];
const SECRET_WORDS: [&str; 3] = ["secret", "credential", "keychain"];
const SECRET_VARS: [&str; 6] = [
    "SECRET",
    "TOKEN",
    "PASSWORD",
    "PASSWD",
    "API_KEY",
    "PRIVATE_KEY",
];
const DELETERS: [&str; 6] = ["rm", "rmdir", "unlink", "shred", "rimraf", "trash"];
const SAFE_DEVICES: [&str; 4] = ["/dev/null", "/dev/stdout", "/dev/stderr", "/dev/stdin"];

pub fn check(command: &str, worktree: &Path, base: &str) -> Option<&'static str> {
    let lowered = command.to_lowercase();
    let squeezed = lowered.split_whitespace().collect::<Vec<_>>().join(" ");
    if DATA_PHRASES.iter().any(|p| squeezed.contains(p)) {
        return Some("it drops or deletes data");
    }
    if SECRET_PHRASES.iter().any(|p| squeezed.contains(p)) {
        return Some("it reads or writes secrets or credentials");
    }
    let words = tokens(command);
    if words.iter().any(|w| touches_secrets(w)) {
        return Some("it reads or writes secrets or credentials");
    }
    if force_pushes(&words, base) {
        return Some("it force-pushes to main or the base branch");
    }
    let roots = roots_of(worktree);
    if words.iter().any(|w| DELETERS.contains(&w.as_str())) && words.iter().any(|w| unresolved(w))
    {
        return Some("it deletes something it cannot tie to the worktree");
    }
    if words.iter().any(|w| outside(w, worktree, &roots)) {
        return Some("it touches a path outside the worktree");
    }
    None
}

fn tokens(text: &str) -> Vec<String> {
    let split = |c: char| c.is_whitespace() || "\"'`;&|()<>={},".contains(c);
    text.split(split)
        .filter(|t| !t.is_empty())
        .map(String::from)
        .collect()
}

fn touches_secrets(word: &str) -> bool {
    if let Some(var) = word.strip_prefix('$') {
        let var = var.trim_matches(|c| c == '{' || c == '}').to_uppercase();
        return SECRET_VARS.iter().any(|v| var.contains(v));
    }
    let lowered = word.to_lowercase();
    lowered.split('/').any(|part| {
        SECRET_NAMES.contains(&part)
            || part.starts_with(".env.")
            || SECRET_SUFFIXES.iter().any(|s| part.ends_with(s))
            || SECRET_WORDS.iter().any(|w| part.contains(w))
    })
}

fn force_pushes(words: &[String], base: &str) -> bool {
    let Some(at) = words.iter().position(|w| w == "push") else {
        return false;
    };
    if !words[..at].iter().any(|w| w == "git") {
        return false;
    }
    let args = &words[at + 1..];
    let short_force = |w: &String| w.starts_with('-') && !w.starts_with("--") && w.contains('f');
    if args.iter().any(|w| w == "--mirror") {
        return true;
    }
    let forced = args
        .iter()
        .any(|w| short_force(w) || w.starts_with("--force") || w == "--delete" || w.starts_with('+'));
    if !forced {
        return false;
    }
    if args.iter().any(|w| w == "--all") {
        return true;
    }
    args.iter().any(|w| {
        w.split(':').any(|part| {
            let name = part.trim_start_matches('+');
            let name = name.strip_prefix("refs/heads/").unwrap_or(name);
            !name.is_empty() && (name == "main" || name == "master" || name == base)
        })
    })
}

fn roots_of(worktree: &Path) -> Vec<PathBuf> {
    let mut roots = vec![normalize(worktree)];
    if let Ok(real) = worktree.canonicalize() {
        roots.push(real);
    }
    roots
}

fn normalize(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for part in path.components() {
        match part {
            Component::ParentDir => {
                out.pop();
            }
            Component::CurDir => {}
            other => out.push(other),
        }
    }
    out
}

fn unresolved(word: &str) -> bool {
    word.starts_with('$') || word.starts_with('~')
}

fn outside(word: &str, worktree: &Path, roots: &[PathBuf]) -> bool {
    if word.starts_with('~') || word.starts_with("$HOME") || word.starts_with("${HOME") {
        return true;
    }
    if SAFE_DEVICES.contains(&word) {
        return false;
    }
    let path = Path::new(word);
    let resolved = if path.is_absolute() {
        normalize(path)
    } else if word == ".." || word.starts_with("../") || word.contains("/../") {
        normalize(&worktree.join(path))
    } else {
        return false;
    };
    !roots.iter().any(|root| resolved.starts_with(root))
}

#[cfg(test)]
mod tests {
    use super::*;

    const WT: &str = "/h/.townhall/worktrees/shop-1/fix";

    fn denied(command: &str) -> Option<&'static str> {
        check(command, Path::new(WT), "main")
    }

    #[test]
    fn deletes_outside_the_worktree_are_denied() {
        for command in [
            "rm -rf /",
            "rm -rf /Users/me/code",
            "rm -rf ~/Documents",
            "rm -rf $HOME/x",
            "rm -rf ../shop-2",
            "rm -rf build/../../other",
            "rm -rf $TARGET",
            "rm -r /h/.townhall/worktrees/shop-1/other",
            "unlink /etc/hosts",
            "cd build && rm -rf /tmp/x",
        ] {
            assert!(denied(command).is_some(), "{command}");
        }
    }

    #[test]
    fn deletes_inside_the_worktree_are_left_to_the_adviser() {
        for command in [
            "rm -rf build",
            "rm -rf ./target/debug",
            "rm -f src/old.rs",
            &format!("rm -rf {WT}/build"),
        ] {
            assert_eq!(denied(command), None, "{command}");
        }
    }

    #[test]
    fn force_pushes_to_main_or_the_base_branch_are_denied() {
        for command in [
            "git push --force origin main",
            "git push -f origin main",
            "git push --force-with-lease origin main",
            "git push --force-with-lease=main origin main",
            "git push origin +main",
            "git push origin +HEAD:main",
            "git push -f origin HEAD:refs/heads/master",
            "git push --force --all",
            "git push --mirror",
        ] {
            assert!(denied(command).is_some(), "{command}");
        }
        assert!(check("git push -f origin develop", Path::new(WT), "develop").is_some());
    }

    #[test]
    fn other_pushes_are_left_to_the_adviser() {
        for command in [
            "git push origin townhall/fix",
            "git push --force-with-lease origin townhall/fix",
            "git push -u origin HEAD",
            "git push origin main",
        ] {
            assert_eq!(denied(command), None, "{command}");
        }
    }

    #[test]
    fn secrets_credentials_and_keychains_are_denied() {
        for command in [
            "cat ~/.ssh/id_rsa",
            "cat .env",
            "cat .env.production",
            "cat config/credentials.json",
            "echo $GITHUB_TOKEN",
            "security find-generic-password -s github",
            "gh auth token",
            "printenv",
            "cat server.pem",
            "cat secrets.yml",
            "git credential fill",
            "echo hi > .aws/config",
        ] {
            assert!(denied(command).is_some(), "{command}");
        }
    }

    #[test]
    fn dropping_or_deleting_data_is_denied() {
        for command in [
            "psql -c 'DROP DATABASE shop'",
            "sqlite3 app.db 'drop table users'",
            "psql -c \"delete from users\"",
            "dropdb shop",
            "rails db:drop",
            "redis-cli FLUSHALL",
            "docker volume rm pgdata",
            "psql -c 'TRUNCATE TABLE orders'",
        ] {
            assert!(denied(command).is_some(), "{command}");
        }
    }

    #[test]
    fn paths_outside_the_worktree_are_denied() {
        for command in [
            "cat /etc/hosts",
            "cargo build --manifest-path /Users/me/other/Cargo.toml",
            "git -C /Users/me/other status",
            "cd ../other && ls",
            "ls ~",
            "cat --file=/Users/me/x",
        ] {
            assert!(denied(command).is_some(), "{command}");
        }
    }

    #[test]
    fn ordinary_commands_in_the_worktree_pass() {
        for command in [
            "cargo test -p townhall-core",
            "git status",
            "git diff --stat",
            "npm run build",
            "ls src",
            "cat src/main.rs",
            "grep -rn foo src > /dev/null 2>&1",
            "gh pr view 12",
            "curl https://example.com/a/b",
            &format!("cat {WT}/src/main.rs"),
            "cat ./docs/../README.md",
            "",
        ] {
            assert_eq!(denied(command), None, "{command}");
        }
    }
}
