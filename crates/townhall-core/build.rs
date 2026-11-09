use std::process::Command;

fn git(args: &[&str]) -> Option<String> {
    let out = Command::new("git").args(args).output().ok()?;
    if !out.status.success() {
        return None;
    }
    let text = String::from_utf8(out.stdout).ok()?.trim().to_string();
    (!text.is_empty()).then_some(text)
}

fn main() {
    println!("cargo:rerun-if-env-changed=TOWNHALL_RELEASE");
    let release = std::env::var_os("TOWNHALL_RELEASE").is_some_and(|v| !v.is_empty());
    let sha = if release {
        None
    } else {
        git(&["rev-parse", "--short", "HEAD"])
    };
    if sha.is_some() {
        if let Some(head) = git(&["rev-parse", "--git-path", "HEAD"]) {
            println!("cargo:rerun-if-changed={head}");
        }
        if let Some(branch) = git(&["symbolic-ref", "-q", "HEAD"]) {
            if let Some(path) = git(&["rev-parse", "--git-path", &branch]) {
                println!("cargo:rerun-if-changed={path}");
            }
        }
    }
    let base = std::env::var("CARGO_PKG_VERSION").expect("cargo sets CARGO_PKG_VERSION");
    let version = match sha {
        Some(sha) => format!("{base}+{sha}"),
        None => base,
    };
    println!("cargo:rustc-env=TOWNHALL_VERSION={version}");
}
