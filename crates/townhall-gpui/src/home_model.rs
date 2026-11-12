use townhall_core::model::Repo;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimeOfDay {
    Morning,
    Afternoon,
    Evening,
}

pub const PREVIEW_STARTERS: [&str; 3] = [
    "townhall needs a settings page",
    "fix the failing tests on shop main",
    "find slow queries in search",
];

pub const NO_REPOS_TEXT: &str =
    "townhall works in git worktrees of your repos. it never writes into a repo itself.";

const MORNING_FROM: u32 = 5;
const AFTERNOON_FROM: u32 = 12;
const EVENING_FROM: u32 = 18;

pub fn time_of_day(hour: u32) -> TimeOfDay {
    if (MORNING_FROM..AFTERNOON_FROM).contains(&hour) {
        TimeOfDay::Morning
    } else if (AFTERNOON_FROM..EVENING_FROM).contains(&hour) {
        TimeOfDay::Afternoon
    } else {
        TimeOfDay::Evening
    }
}

pub fn first_name(user_name: Option<&str>) -> Option<String> {
    user_name?.split_whitespace().next().map(String::from)
}

pub fn greeting_text(hour: u32, user_name: Option<&str>) -> String {
    let words = match time_of_day(hour) {
        TimeOfDay::Morning => "morning",
        TimeOfDay::Afternoon => "afternoon",
        TimeOfDay::Evening => "evening",
    };
    match first_name(user_name) {
        Some(name) => format!("good {words}, {name}"),
        None => format!("good {words}"),
    }
}

pub fn starter_prompts(repos: Option<&[Repo]>) -> Vec<String> {
    let repos = repos.unwrap_or(&[]);
    if repos.is_empty() {
        return PREVIEW_STARTERS.iter().map(|s| s.to_string()).collect();
    }
    (0..3)
        .map(|i| {
            let repo = &repos[i % repos.len()];
            match i {
                0 => format!(
                    "fix the failing tests on {} {}",
                    repo.name, repo.base_branch
                ),
                1 => format!("find dead code in {}", repo.name),
                _ => format!("tidy the readme in {}", repo.name),
            }
        })
        .collect()
}

pub fn repos_known_label(count: usize) -> String {
    format!(
        "{count} {} known",
        if count == 1 { "repo" } else { "repos" }
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testkit::repo;

    #[test]
    fn morning_runs_from_5_to_noon() {
        assert_eq!(time_of_day(5), TimeOfDay::Morning);
        assert_eq!(time_of_day(11), TimeOfDay::Morning);
    }

    #[test]
    fn afternoon_runs_from_noon_to_18() {
        assert_eq!(time_of_day(12), TimeOfDay::Afternoon);
        assert_eq!(time_of_day(17), TimeOfDay::Afternoon);
    }

    #[test]
    fn evening_runs_from_18_through_the_night() {
        assert_eq!(time_of_day(18), TimeOfDay::Evening);
        assert_eq!(time_of_day(23), TimeOfDay::Evening);
        assert_eq!(time_of_day(2), TimeOfDay::Evening);
    }

    #[test]
    fn the_first_name_is_the_first_word_with_its_capitals() {
        assert_eq!(
            first_name(Some("William Thøgersen")).as_deref(),
            Some("William")
        );
        assert_eq!(
            first_name(Some("  McKenzie  Lee ")).as_deref(),
            Some("McKenzie")
        );
    }

    #[test]
    fn there_is_no_first_name_without_a_usable_name() {
        assert_eq!(first_name(None), None);
        assert_eq!(first_name(Some("   ")), None);
    }

    #[test]
    fn the_greeting_uses_the_first_name_when_there_is_one() {
        assert_eq!(
            greeting_text(14, Some("William Thøgersen")),
            "good afternoon, William"
        );
        assert_eq!(greeting_text(9, None), "good morning");
    }

    #[test]
    fn starters_use_the_previews_when_no_repo_is_known() {
        let preview: Vec<String> = PREVIEW_STARTERS.iter().map(|s| s.to_string()).collect();
        assert_eq!(starter_prompts(None), preview);
        assert_eq!(starter_prompts(Some(&[])), preview);
    }

    #[test]
    fn starters_name_the_known_repos_and_their_base_branch() {
        let repos = [
            repo("townhall", "main"),
            repo("shop", "develop"),
            repo("data-hub", "main"),
        ];
        assert_eq!(
            starter_prompts(Some(&repos)),
            [
                "fix the failing tests on townhall main",
                "find dead code in shop",
                "tidy the readme in data-hub"
            ]
        );
    }

    #[test]
    fn starters_reuse_repos_when_fewer_than_three_are_known() {
        let repos = [repo("shop", "develop")];
        assert_eq!(
            starter_prompts(Some(&repos)),
            [
                "fix the failing tests on shop develop",
                "find dead code in shop",
                "tidy the readme in shop"
            ]
        );
    }

    #[test]
    fn the_repos_known_label_counts() {
        assert_eq!(repos_known_label(1), "1 repo known");
        assert_eq!(repos_known_label(3), "3 repos known");
    }
}
