//! The code rule's search: what each repo has for each hint, by tracked path
//! first and file content second, within a time budget per repo.

use std::path::{Path, PathBuf};
use std::time::Duration;

use tokio::time::Instant;

use super::git::{self, Grep};
use super::hints::Hint;
use super::score::Hits;
use super::{parallel, Candidate};
use crate::Result;

const REPO_BUDGET: Duration = Duration::from_secs(3);

/// Hits for every candidate, in candidate order. A repo that runs out of time
/// keeps the hits it found so far.
pub(crate) async fn search_all(candidates: &[Candidate], hints: &[Hint]) -> Result<Vec<Hits>> {
    let jobs = candidates
        .iter()
        .map(|c| search_repo(c.path.clone(), hints.to_vec(), Instant::now() + REPO_BUDGET))
        .collect();
    parallel::join_all(jobs).await
}

async fn search_repo(dir: PathBuf, hints: Vec<Hint>, deadline: Instant) -> Result<Hits> {
    let Some(files) = git::ls_files(&dir, deadline).await? else {
        return Ok(vec![None; hints.len()]);
    };
    let tracked = Tracked::new(files);
    let mut hits = Vec::with_capacity(hints.len());
    for hint in &hints {
        let by_path = hint.path_needle().and_then(|needle| tracked.hit(&needle));
        let hit = match by_path {
            Some(path) => Some(path),
            None => content_hit(&dir, &hint.text, deadline).await?,
        };
        hits.push(hit);
    }
    Ok(hits)
}

async fn content_hit(dir: &Path, text: &str, deadline: Instant) -> Result<Option<String>> {
    Ok(match git::grep_first(dir, text, deadline).await? {
        Grep::Hit(file) => Some(format!("{text} in {file}")),
        Grep::Miss | Grep::TimedOut => None,
    })
}

/// A repo's tracked paths, with ASCII-lowercase copies (same byte offsets).
pub(crate) struct Tracked {
    files: Vec<String>,
    lower: Vec<String>,
}

impl Tracked {
    pub fn new(files: Vec<String>) -> Self {
        let lower = files.iter().map(|f| f.to_ascii_lowercase()).collect();
        Self { files, lower }
    }

    /// The first path that has the lowercase `needle` as whole path parts:
    /// the file itself, or the folder up to the needle with a trailing `/`.
    pub fn hit(&self, needle: &str) -> Option<String> {
        let suffix = format!("/{needle}");
        let prefix = format!("{needle}/");
        let inner = format!("/{prefix}");
        self.files
            .iter()
            .zip(&self.lower)
            .find_map(|(file, lower)| {
                if lower == needle || lower.ends_with(&suffix) {
                    return Some(file.clone());
                }
                let end = if lower.starts_with(&prefix) {
                    prefix.len()
                } else {
                    lower.find(&inner)? + inner.len()
                };
                Some(file[..end].to_string())
            })
    }
}

#[cfg(test)]
mod tests {
    use tempfile::TempDir;
    use tokio::time::Instant;

    use super::{search_repo, Tracked};
    use crate::cmd::run;
    use crate::repo_infer::hints::{Hint, HintKind};

    fn tracked() -> Tracked {
        Tracked::new(
            ["README.md", "web/src/Search/query.ts", "web/package.json"]
                .map(String::from)
                .to_vec(),
        )
    }

    #[test]
    fn file_names_and_paths_match_whole_parts_in_any_case() {
        let t = tracked();
        let hit = |needle: &str| t.hit(needle);
        assert_eq!(hit("query.ts").as_deref(), Some("web/src/Search/query.ts"));
        assert_eq!(
            hit("src/search/query.ts").as_deref(),
            Some("web/src/Search/query.ts")
        );
        assert_eq!(hit("readme.md").as_deref(), Some("README.md"));
        assert_eq!(hit("src/search").as_deref(), Some("web/src/Search/"));
        assert_eq!(hit("web").as_deref(), Some("web/"));
        assert_eq!(hit("ery.ts"), None);
        assert_eq!(hit("search/query"), None);
        assert_eq!(hit("package"), None);
    }

    #[tokio::test]
    async fn a_repo_out_of_time_has_no_hits() {
        let dir = TempDir::new().unwrap();
        run("git", &["init", "-b", "main"], Some(dir.path()))
            .await
            .unwrap();
        let hint = Hint {
            text: "anything.ts".to_string(),
            kind: HintKind::File,
        };
        let hits = search_repo(dir.path().to_path_buf(), vec![hint], Instant::now())
            .await
            .unwrap();
        assert_eq!(hits, vec![None]);
    }
}
