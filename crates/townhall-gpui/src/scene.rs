use crate::theme::Mode;

pub const NOT_BUILT_EXIT: i32 = 3;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Route {
    Conversation(String),
    NotBuilt(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Scene {
    pub size: (f32, f32),
    pub mode: Option<Mode>,
    pub route: Option<Route>,
}

impl Default for Scene {
    fn default() -> Self {
        Self {
            size: (1200.0, 800.0),
            mode: None,
            route: None,
        }
    }
}

pub fn parse_size(text: &str) -> Result<(f32, f32), String> {
    let bad = || format!("--size needs WxH in pixels, got {text}");
    let (w, h) = text.split_once('x').ok_or_else(bad)?;
    let w: f32 = w.parse().map_err(|_| bad())?;
    let h: f32 = h.parse().map_err(|_| bad())?;
    if w < 200.0 || h < 200.0 {
        return Err(format!("--size {text} is below 200x200"));
    }
    Ok((w, h))
}

pub fn parse_mode(text: &str) -> Result<Mode, String> {
    match text {
        "dark" => Ok(Mode::Dark),
        "light" => Ok(Mode::Light),
        other => Err(format!("--theme needs dark or light, got {other}")),
    }
}

pub fn parse_route(text: &str) -> Route {
    match text.strip_prefix("/c/") {
        Some(id) if !id.is_empty() => Route::Conversation(id.to_string()),
        _ => Route::NotBuilt(text.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn size_is_width_x_height() {
        assert_eq!(parse_size("1200x800"), Ok((1200.0, 800.0)));
        assert_eq!(parse_size("400x800"), Ok((400.0, 800.0)));
    }

    #[test]
    fn a_malformed_or_tiny_size_is_an_error_naming_the_input() {
        assert!(parse_size("1200").unwrap_err().contains("1200"));
        assert!(parse_size("axb").unwrap_err().contains("axb"));
        assert!(parse_size("10x10").unwrap_err().contains("10x10"));
    }

    #[test]
    fn theme_is_dark_or_light() {
        assert_eq!(parse_mode("dark"), Ok(Mode::Dark));
        assert_eq!(parse_mode("light"), Ok(Mode::Light));
        assert!(parse_mode("system").unwrap_err().contains("system"));
    }

    #[test]
    fn a_conversation_path_selects_that_conversation() {
        assert_eq!(parse_route("/c/abc"), Route::Conversation("abc".into()));
    }

    #[test]
    fn every_other_path_is_not_built_yet() {
        for path in ["/", "/settings", "/townhall", "/c/", "home"] {
            assert_eq!(parse_route(path), Route::NotBuilt(path.into()));
        }
    }
}
