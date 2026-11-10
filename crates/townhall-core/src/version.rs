//! The build's version: the workspace version, plus `+<short sha>` on builds from a git checkout.

/// `0.2.0-alpha.1+d812d0d` in a dev build, `0.2.0-alpha.1` in a release build or without git.
pub const VERSION: &str = env!("TOWNHALL_VERSION");

#[cfg(test)]
mod tests {
    use super::VERSION;

    #[test]
    fn starts_with_the_cargo_version() {
        assert!(VERSION.starts_with(env!("CARGO_PKG_VERSION")), "{VERSION}");
    }

    #[test]
    fn metadata_is_a_short_hex_sha() {
        let rest = &VERSION[env!("CARGO_PKG_VERSION").len()..];
        if !rest.is_empty() {
            let sha = rest.strip_prefix('+').expect(VERSION);
            assert!(
                sha.len() >= 7 && sha.chars().all(|c| c.is_ascii_hexdigit()),
                "{VERSION}"
            );
        }
    }
}
