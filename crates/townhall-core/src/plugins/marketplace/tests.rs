use serde_json::{json, Value};

use super::*;
use crate::Error;

const SHA: &str = "3f9c0d1e5b7a4c2d8e6f1a0b9c8d7e6f5a4b3c2d";

fn base() -> Value {
    json!({
        "name": "acme",
        "owner": { "name": "Acme" },
        "plugins": [{ "name": "one", "source": "./" }]
    })
}

fn with_source(source: Value) -> Value {
    let mut market = base();
    market["plugins"][0]["source"] = source;
    market
}

fn with_entry(key: &str, value: Value) -> Value {
    let mut market = base();
    market["plugins"][0][key] = value;
    market
}

fn refused(market: &Value) -> String {
    match parse(&market.to_string()) {
        Ok(parsed) => panic!("accepted {market}: {parsed:?}"),
        Err(e) => {
            assert!(matches!(e, Error::InvalidInput(_)), "{e:?}");
            e.to_string()
        }
    }
}

fn source_of(market: &Value) -> Source {
    parse(&market.to_string()).unwrap().plugins.remove(0).source
}

#[test]
fn the_three_real_marketplaces_decode() {
    let adhd = parse(include_str!(
        "../../../tests/fixtures/plugins/marketplaces/i-have-adhd.json"
    ))
    .unwrap();
    assert_eq!(adhd.name, "i-have-adhd");
    assert_eq!(adhd.owner.name, "Ayoub G.");
    assert!(adhd.description.unwrap().starts_with("Shape Claude Code"));
    assert_eq!(adhd.plugins.len(), 1);
    assert_eq!(adhd.plugins[0].source, Source::Path("./".into()));
    assert_eq!(adhd.plugins[0].category.as_deref(), Some("productivity"));

    let typesafe = parse(include_str!(
        "../../../tests/fixtures/plugins/marketplaces/typesafe-ai.json"
    ))
    .unwrap();
    assert_eq!(typesafe.owner.url.as_deref(), Some("https://typesafe.ai"));
    assert_eq!(typesafe.plugins[0].name, "typesafe");

    let ui = parse(include_str!(
        "../../../tests/fixtures/plugins/marketplaces/ui-ux-pro-max-skill.json"
    ))
    .unwrap();
    assert_eq!(ui.id.as_deref(), Some("ui-ux-pro-max-skill"));
    assert_eq!(ui.description, None);
    assert_eq!(ui.metadata.unwrap().version.as_deref(), Some("2.15.0"));
    let entry = &ui.plugins[0];
    assert_eq!(entry.version.as_deref(), Some("2.15.0"));
    assert_eq!(entry.author.as_ref().unwrap().name, "nextlevelbuilder");
    assert_eq!(entry.keywords.len(), 9);
    assert_eq!(entry.strict, Some(false));
    assert_eq!(entry.category.as_deref(), Some("design"));
}

#[test]
fn the_schema_key_is_accepted() {
    let mut market = base();
    market["$schema"] = json!("https://www.schemastore.org/claude-code-marketplace.json");
    parse(&market.to_string()).unwrap();
}

#[test]
fn every_source_form_of_the_design_decodes() {
    let market = parse(include_str!(
        "../../../tests/fixtures/plugins/marketplaces/acme-plugins.json"
    ))
    .unwrap();
    assert_eq!(
        market.description.as_deref(),
        Some("Plugins of the Acme team")
    );
    let sources: Vec<_> = market.plugins.iter().map(|p| p.source.clone()).collect();
    assert_eq!(
        sources,
        [
            Source::Github {
                repo: "acme/townhall-release-notes".into(),
                git_ref: Some("v0.3.0".into()),
                sha: Some(SHA.into()),
            },
            Source::Path("./plugins/theme-paper".into()),
            Source::Url {
                url: "https://gitlab.example.com/acme/tools.git".into(),
                git_ref: None,
                sha: None,
            },
            Source::Url {
                url: "git@git.example.com:acme/tools.git".into(),
                git_ref: Some("main".into()),
                sha: None,
            },
            Source::GitSubdir {
                url: "https://github.com/acme/mono.git".into(),
                path: "plugins/tools".into(),
                git_ref: None,
                sha: Some(SHA.into()),
            },
            Source::Github {
                repo: "acme/claude-only".into(),
                git_ref: None,
                sha: None,
            },
        ]
    );
}

#[test]
fn an_entry_decodes_its_townhall_extension_and_tags() {
    let market = parse(include_str!(
        "../../../tests/fixtures/plugins/marketplaces/acme-plugins.json"
    ))
    .unwrap();
    let notes = &market.plugins[0];
    assert_eq!(notes.version.as_deref(), Some("0.3.0"));
    assert_eq!(notes.category.as_deref(), Some("workflow"));
    assert_eq!(notes.tags, ["release", "docs"]);
    assert_eq!(notes.townhall, None);
    let wrapped = market.plugins[5].townhall.as_ref().unwrap();
    assert_eq!(wrapped.api_version, 1);
    assert_eq!(
        wrapped.permissions.claude.hooks.as_ref().unwrap().events,
        ["PostToolUse"]
    );
}

#[test]
fn an_unknown_top_level_key_is_refused() {
    let mut market = base();
    market["themes"] = json!([]);
    let why = refused(&market);
    assert!(why.contains("themes"), "{why}");
}

#[test]
fn an_unknown_key_on_an_entry_is_refused() {
    let why = refused(&with_entry("catagory", json!("x")));
    assert!(why.contains("catagory"), "{why}");
}

#[test]
fn an_unknown_key_in_the_owner_is_refused() {
    let mut market = base();
    market["owner"]["phone"] = json!("123");
    let why = refused(&market);
    assert!(why.contains("phone"), "{why}");
}

#[test]
fn an_unknown_key_in_a_source_object_is_refused() {
    let why = refused(&with_source(
        json!({ "source": "github", "repo": "a/b", "branch": "main" }),
    ));
    assert!(why.contains("branch"), "{why}");
}

#[test]
fn an_unknown_key_in_the_townhall_extension_is_refused() {
    let why = refused(&with_entry(
        "townhall",
        json!({ "apiVersion": 1, "permisions": {} }),
    ));
    assert!(why.contains("permisions"), "{why}");
}

#[test]
fn a_townhall_extension_checks_its_permissions() {
    let why = refused(&with_entry(
        "townhall",
        json!({ "apiVersion": 1, "permissions": { "api": ["nope"] } }),
    ));
    assert!(why.contains("nope"), "{why}");
}

#[test]
fn a_townhall_extension_needs_a_positive_api_version() {
    let why = refused(&with_entry("townhall", json!({ "apiVersion": 0 })));
    assert!(why.contains("apiVersion"), "{why}");
}

#[test]
fn a_bad_marketplace_name_is_refused() {
    for name in ["", "-x", "a b", "a/b", "a@b", &"a".repeat(65)] {
        let mut market = base();
        market["name"] = json!(name);
        let why = refused(&market);
        assert!(why.contains("marketplace name"), "{name}: {why}");
    }
}

#[test]
fn a_bad_plugin_name_is_refused() {
    for name in ["", ".x", "a b", "a/b", "a@b", &"a".repeat(65)] {
        let why = refused(&with_entry("name", json!(name)));
        assert!(why.contains("plugin name"), "{name}: {why}");
    }
}

#[test]
fn plugin_names_must_be_unique_ignoring_case() {
    let mut market = base();
    market["plugins"] = json!([
        { "name": "Notes", "source": "./a" },
        { "name": "notes", "source": "./b" }
    ]);
    let why = refused(&market);
    assert!(why.contains("notes"), "{why}");
}

#[test]
fn a_missing_source_is_refused() {
    let mut market = base();
    market["plugins"] = json!([{ "name": "one" }]);
    let why = refused(&market);
    assert!(why.contains("source"), "{why}");
}

#[test]
fn a_missing_owner_is_refused() {
    let why = refused(&json!({ "name": "acme", "plugins": [] }));
    assert!(why.contains("owner"), "{why}");
}

#[test]
fn a_missing_plugin_list_is_refused() {
    let why = refused(&json!({ "name": "acme", "owner": { "name": "A" } }));
    assert!(why.contains("plugins"), "{why}");
}

#[test]
fn an_empty_plugin_list_is_accepted() {
    let market = parse(r#"{"name":"acme","owner":{"name":"A"},"plugins":[]}"#).unwrap();
    assert!(market.plugins.is_empty());
}

#[test]
fn a_plugin_version_that_is_not_semver_is_refused() {
    let why = refused(&with_entry("version", json!("latest")));
    assert!(why.contains("version"), "{why}");
}

#[test]
fn an_entry_link_that_is_not_a_web_address_is_refused() {
    for key in ["homepage", "repository"] {
        let why = refused(&with_entry(key, json!("javascript:alert(1)")));
        assert!(why.contains(key), "{key}: {why}");
    }
}

#[test]
fn an_owner_url_that_is_not_a_web_address_is_refused() {
    let mut market = base();
    market["owner"]["url"] = json!("file:///x");
    let why = refused(&market);
    assert!(why.contains("owner.url"), "{why}");
}

#[test]
fn a_local_path_source_without_the_dot_slash_is_refused() {
    for path in ["plugins/a", ".", "", ".hidden"] {
        let why = refused(&with_source(json!(path)));
        assert!(why.contains("source"), "{path}: {why}");
    }
}

#[test]
fn a_local_path_source_with_a_parent_directory_is_refused() {
    for path in ["./..", "./a/../../b", "../a"] {
        let why = refused(&with_source(json!(path)));
        assert!(why.contains("source"), "{path}: {why}");
    }
}

#[test]
fn an_absolute_local_path_source_is_refused() {
    for path in ["/etc", "C:\\x", "~/x"] {
        let why = refused(&with_source(json!(path)));
        assert!(why.contains("source"), "{path}: {why}");
    }
}

#[test]
fn a_local_path_source_with_a_backslash_is_refused() {
    let why = refused(&with_source(json!(".\\a")));
    assert!(why.contains("source"), "{why}");
}

#[test]
fn an_unknown_source_kind_is_refused() {
    let why = refused(&with_source(json!({ "source": "npm", "package": "x" })));
    assert!(why.contains("npm"), "{why}");
}

#[test]
fn a_source_object_without_a_kind_is_refused() {
    let why = refused(&with_source(json!({ "repo": "a/b" })));
    assert!(why.contains("source"), "{why}");
}

#[test]
fn a_source_that_is_neither_text_nor_object_is_refused() {
    for source in [json!(1), json!(null), json!(["./"])] {
        let why = refused(&with_source(source));
        assert!(why.contains("source"), "{why}");
    }
}

#[test]
fn a_github_repo_must_be_owner_slash_name() {
    for repo in [
        "acme",
        "a/b/c",
        "/b",
        "a/",
        "a b/c",
        "a/b c",
        "https://github.com/a/b",
        "",
    ] {
        let why = refused(&with_source(json!({ "source": "github", "repo": repo })));
        assert!(why.contains("repo"), "{repo}: {why}");
    }
}

#[test]
fn a_github_source_needs_a_repo() {
    let why = refused(&with_source(json!({ "source": "github" })));
    assert!(why.contains("repo"), "{why}");
}

#[test]
fn a_url_source_must_be_https_or_scp_style_ssh() {
    for url in [
        "http://example.com/a.git",
        "file:///tmp/repo",
        "ext::sh -c id",
        "--upload-pack=x",
        "-oProxyCommand=x",
        "https://exa mple.com/a.git",
        "git@host",
        "",
    ] {
        let why = refused(&with_source(json!({ "source": "url", "url": url })));
        assert!(why.contains("url"), "{url}: {why}");
    }
}

#[test]
fn a_url_source_needs_a_url() {
    let why = refused(&with_source(json!({ "source": "url" })));
    assert!(why.contains("url"), "{why}");
}

#[test]
fn a_sha_must_be_forty_hex_characters() {
    for sha in ["abc", &"g".repeat(40), &"a".repeat(41), ""] {
        let why = refused(&with_source(
            json!({ "source": "github", "repo": "a/b", "sha": sha }),
        ));
        assert!(why.contains("sha"), "{sha}: {why}");
    }
}

#[test]
fn an_uppercase_sha_is_accepted() {
    let sha = SHA.to_uppercase();
    let source = source_of(&with_source(
        json!({ "source": "github", "repo": "a/b", "sha": sha }),
    ));
    assert!(matches!(source, Source::Github { sha: Some(_), .. }));
}

#[test]
fn a_ref_that_could_be_an_option_or_is_malformed_is_refused() {
    for git_ref in ["", "--help", "-x", "a b", "a..b", "a\nb"] {
        let why = refused(&with_source(
            json!({ "source": "github", "repo": "a/b", "ref": git_ref }),
        ));
        assert!(why.contains("ref"), "{git_ref:?}: {why}");
    }
}

#[test]
fn a_git_subdir_path_with_a_parent_directory_is_refused() {
    for path in ["../x", "a/../../x"] {
        let why = refused(&with_source(
            json!({ "source": "git-subdir", "url": "https://h.example/a.git", "path": path }),
        ));
        assert!(why.contains("path"), "{path}: {why}");
    }
}

#[test]
fn an_absolute_git_subdir_path_is_refused() {
    let why = refused(&with_source(
        json!({ "source": "git-subdir", "url": "https://h.example/a.git", "path": "/etc" }),
    ));
    assert!(why.contains("path"), "{why}");
}

#[test]
fn a_git_subdir_source_needs_a_url_and_a_path() {
    let no_path = refused(&with_source(
        json!({ "source": "git-subdir", "url": "https://h.example/a.git" }),
    ));
    assert!(no_path.contains("path"), "{no_path}");
    let no_url = refused(&with_source(json!({ "source": "git-subdir", "path": "a" })));
    assert!(no_url.contains("url"), "{no_url}");
}

#[test]
fn a_git_subdir_checks_its_url_ref_and_sha() {
    let url = refused(&with_source(
        json!({ "source": "git-subdir", "url": "http://h.example/a.git", "path": "a" }),
    ));
    assert!(url.contains("url"), "{url}");
    let sha = refused(&with_source(
        json!({ "source": "git-subdir", "url": "https://h.example/a.git", "path": "a", "sha": "1" }),
    ));
    assert!(sha.contains("sha"), "{sha}");
}

#[test]
fn a_local_source_reads_with_nested_folders() {
    assert_eq!(
        source_of(&with_source(json!("./plugins/a/b"))),
        Source::Path("./plugins/a/b".into())
    );
}

#[test]
fn an_oversize_marketplace_is_refused() {
    let mut market = base();
    market["description"] = json!("x".repeat(MAX_MARKETPLACE_BYTES));
    let err = parse(&market.to_string()).unwrap_err();
    assert!(matches!(err, Error::TooLarge(_)), "{err:?}");
}

#[test]
fn text_that_is_not_json_is_refused() {
    for text in ["", "name: x", "[]", "null", "{\"name\":"] {
        let err = parse(text).unwrap_err();
        assert!(matches!(err, Error::InvalidInput(_)), "{text}: {err:?}");
    }
}

#[test]
fn the_official_claude_marketplace_decodes_with_skills_renames_and_lsp_servers() {
    let market = parse(include_str!(
        "../../../tests/fixtures/plugins/marketplaces/claude-plugins-official.json"
    ))
    .unwrap();
    assert_eq!(market.name, "claude-plugins-official");
    assert_eq!(market.owner.email.as_deref(), Some("support@anthropic.com"));
    let amd = &market.plugins[0];
    assert_eq!(amd.strict, Some(false));
    assert_eq!(amd.skills, ["./local-ai-use", "./serving-llms-on-instinct"]);
    assert!(amd.commands.is_empty());
    let web = market
        .plugins
        .iter()
        .find(|p| p.name == "modern-web-guidance")
        .unwrap();
    assert!(
        matches!(&web.source, Source::Url { url, .. } if url.ends_with("modern-web-guidance.git"))
    );
    assert!(web.skills.is_empty());
}

#[test]
fn a_skill_path_must_stay_inside_the_plugin() {
    for bad in ["../x", "/abs", "./a/../../b"] {
        let text = refused(&with_entry("skills", json!([bad])));
        assert!(text.contains("skills"), "{text}");
    }
    assert!(parse(&with_entry("skills", json!(["./a", "b/c"])).to_string()).is_ok());
    assert!(parse(&with_entry("commands", json!("./cmds")).to_string()).is_ok());
}
