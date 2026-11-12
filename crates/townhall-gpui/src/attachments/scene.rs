use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    ThreadTop,
    Lightbox,
    Drop,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Attach {
    pub state: Option<State>,
    pub files: Vec<PathBuf>,
}

impl Attach {
    pub fn is_empty(&self) -> bool {
        self.state.is_none() && self.files.is_empty()
    }
}

pub fn parse_state(text: &str) -> Result<State, String> {
    match text {
        "thread-top" => Ok(State::ThreadTop),
        "lightbox" => Ok(State::Lightbox),
        "drop" => Ok(State::Drop),
        other => Err(format!(
            "--scene-state needs thread-top, lightbox or drop, got {other}"
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_scene_state_is_one_of_three_names() {
        assert_eq!(parse_state("thread-top"), Ok(State::ThreadTop));
        assert_eq!(parse_state("lightbox"), Ok(State::Lightbox));
        assert_eq!(parse_state("drop"), Ok(State::Drop));
        assert!(parse_state("nope").unwrap_err().contains("nope"));
    }

    #[test]
    fn an_empty_scene_asks_for_nothing() {
        assert!(Attach::default().is_empty());
        let with_file = Attach {
            files: vec!["a.png".into()],
            ..Attach::default()
        };
        assert!(!with_file.is_empty());
    }
}
