pub struct Official {
    pub name: &'static str,
    pub repo: &'static str,
    pub sha: &'static str,
    pub snapshot: &'static str,
}

pub const OFFICIAL: Official = Official {
    name: "townhall-official",
    repo: "WilliamThogersen/townhall-plugins",
    sha: "d999cc02bb20351f320ca3c180e31927cda4871d",
    snapshot: include_str!("official/marketplace.json"),
};

impl Official {
    pub fn is(&self, name: &str) -> bool {
        self.name.eq_ignore_ascii_case(name.trim())
    }

    pub fn url(&self) -> String {
        format!("https://github.com/{}", self.repo)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugins::marketplace;

    #[test]
    fn the_snapshot_parses_and_is_the_official_marketplace() {
        let market = marketplace::parse(OFFICIAL.snapshot).unwrap();
        assert_eq!(market.name, OFFICIAL.name);
        let names: Vec<&str> = market.plugins.iter().map(|p| p.name.as_str()).collect();
        assert_eq!(
            names,
            [
                "theme-slate",
                "starter-prompts",
                "agent-types",
                "sample-full"
            ]
        );
    }

    #[test]
    fn the_pin_is_a_full_sha_and_the_name_is_matched_ignoring_case() {
        assert_eq!(OFFICIAL.sha.len(), 40);
        assert!(OFFICIAL.sha.chars().all(|c| c.is_ascii_hexdigit()));
        assert!(OFFICIAL.is(" Townhall-Official "));
        assert!(!OFFICIAL.is("townhall-official-2"));
        assert_eq!(
            OFFICIAL.url(),
            "https://github.com/WilliamThogersen/townhall-plugins"
        );
    }
}
