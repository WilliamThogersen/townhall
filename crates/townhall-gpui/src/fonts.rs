use std::borrow::Cow;

use gpui::App;

pub const REGULAR: &[u8] = include_bytes!("../assets/fonts/JetBrainsMono-Regular.ttf");
pub const BOLD: &[u8] = include_bytes!("../assets/fonts/JetBrainsMono-Bold.ttf");
#[cfg(test)]
const LICENSE: &str = include_str!("../assets/fonts/OFL.txt");

pub fn register(cx: &App) -> Result<(), String> {
    cx.text_system()
        .add_fonts(vec![Cow::Borrowed(REGULAR), Cow::Borrowed(BOLD)])
        .map_err(|e| format!("cannot load JetBrains Mono: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_embedded_files_are_truetype_and_licensed() {
        for bytes in [REGULAR, BOLD] {
            assert_eq!(&bytes[..4], &[0, 1, 0, 0]);
        }
        assert!(LICENSE.contains("SIL OPEN FONT LICENSE"));
    }
}
