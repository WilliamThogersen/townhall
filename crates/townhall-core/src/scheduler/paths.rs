pub fn matches(pattern: &str, path: &str) -> bool {
    let pattern: Vec<&str> = pattern.split('/').collect();
    let path: Vec<&str> = path.split('/').collect();
    segments(&pattern, &path)
}

fn segments(pattern: &[&str], path: &[&str]) -> bool {
    match pattern.split_first() {
        None => path.is_empty(),
        Some((&"**", [])) => !path.is_empty(),
        Some((&"**", rest)) => (0..=path.len()).any(|skip| segments(rest, &path[skip..])),
        Some((first, rest)) => match path.split_first() {
            Some((name, tail)) => {
                let first: Vec<char> = first.chars().collect();
                let name: Vec<char> = name.chars().collect();
                within(&first, &name) && segments(rest, tail)
            }
            None => false,
        },
    }
}

fn within(pattern: &[char], name: &[char]) -> bool {
    match pattern.split_first() {
        None => name.is_empty(),
        Some(('*', rest)) => (0..=name.len()).any(|skip| within(rest, &name[skip..])),
        Some(('?', rest)) => name
            .split_first()
            .is_some_and(|(_, tail)| within(rest, tail)),
        Some((c, rest)) => name
            .split_first()
            .is_some_and(|(n, tail)| n == c && within(rest, tail)),
    }
}

pub fn any_match(patterns: &[String], files: &[String]) -> bool {
    files.iter().any(|f| patterns.iter().any(|p| matches(p, f)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_literal_matches_only_itself() {
        assert!(matches("README.md", "README.md"));
        assert!(!matches("README.md", "docs/README.md"));
        assert!(!matches("README.md", "README.mdx"));
    }

    #[test]
    fn a_star_stays_inside_one_segment() {
        assert!(matches("src/*.rs", "src/main.rs"));
        assert!(!matches("src/*.rs", "src/a/main.rs"));
        assert!(matches("*", "a"));
        assert!(!matches("*", "a/b"));
        assert!(matches("a*c", "abbc"));
        assert!(matches("a*c", "ac"));
    }

    #[test]
    fn a_question_mark_is_one_character_but_not_a_slash() {
        assert!(matches("a?c", "abc"));
        assert!(!matches("a?c", "ac"));
        assert!(!matches("a?c", "a/c"));
    }

    #[test]
    fn a_double_star_segment_spans_any_depth() {
        assert!(matches("**/migrations/**", "db/migrations/1.sql"));
        assert!(matches("**/migrations/**", "migrations/1.sql"));
        assert!(matches("**/migrations/**", "a/b/migrations/c/d.sql"));
        assert!(!matches("**/migrations/**", "README.md"));
        assert!(!matches("**/migrations/**", "db/migrations"));
        assert!(matches("src/**", "src/a/b.rs"));
        assert!(matches("**", "a/b/c"));
        assert!(matches("a/**/b", "a/b"));
        assert!(matches("a/**/b", "a/x/y/b"));
    }

    #[test]
    fn a_double_star_inside_a_segment_acts_as_a_single_star() {
        assert!(matches("a**c", "abc"));
        assert!(!matches("a**c", "a/b/c"));
    }

    #[test]
    fn any_match_needs_one_file_and_one_pattern() {
        let patterns = vec!["**/auth/**".to_string(), "*.sql".to_string()];
        let files = vec!["README.md".to_string(), "x.sql".to_string()];
        assert!(any_match(&patterns, &files));
        assert!(!any_match(&patterns, &["README.md".to_string()]));
        assert!(!any_match(&[], &files));
        assert!(!any_match(&patterns, &[]));
    }
}
