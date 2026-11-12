use gpui::Rgba;

use crate::theme::Theme;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Glyph {
    Alive,
    Dead,
    Working,
    Waiting,
    Error,
    Queued,
    Ci,
    Merged,
}

impl Glyph {
    pub fn symbol(self) -> &'static str {
        match self {
            Glyph::Alive => "●",
            Glyph::Dead => "○",
            Glyph::Working => "⠋",
            Glyph::Waiting => "◆",
            Glyph::Error => "×",
            Glyph::Queued => "◌",
            Glyph::Ci => "◐",
            Glyph::Merged => "✓",
        }
    }

    pub fn color(self, theme: &Theme) -> Rgba {
        match self {
            Glyph::Alive => theme.status_alive(),
            Glyph::Working | Glyph::Ci => theme.status_working(),
            Glyph::Waiting => theme.status_waiting(),
            Glyph::Error => theme.status_error(),
            Glyph::Dead | Glyph::Queued => theme.status_dead(),
            Glyph::Merged => theme.palette.ok,
        }
    }

    pub fn bold(self) -> bool {
        matches!(self, Glyph::Error | Glyph::Merged)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::Mode;

    #[test]
    fn every_glyph_has_its_style_guide_symbol() {
        let table = [
            (Glyph::Alive, "●"),
            (Glyph::Dead, "○"),
            (Glyph::Working, "⠋"),
            (Glyph::Waiting, "◆"),
            (Glyph::Error, "×"),
            (Glyph::Queued, "◌"),
            (Glyph::Ci, "◐"),
            (Glyph::Merged, "✓"),
        ];
        for (glyph, symbol) in table {
            assert_eq!(glyph.symbol(), symbol, "{glyph:?}");
        }
    }

    #[test]
    fn colors_follow_the_status_tokens() {
        let t = Theme::new(Mode::Dark);
        assert_eq!(Glyph::Alive.color(&t), t.status_alive());
        assert_eq!(Glyph::Working.color(&t), t.status_working());
        assert_eq!(Glyph::Ci.color(&t), t.status_working());
        assert_eq!(Glyph::Waiting.color(&t), t.status_waiting());
        assert_eq!(Glyph::Error.color(&t), t.status_error());
        assert_eq!(Glyph::Dead.color(&t), t.status_dead());
        assert_eq!(Glyph::Queued.color(&t), t.status_dead());
        assert_eq!(Glyph::Merged.color(&t), t.palette.ok);
    }

    #[test]
    fn only_error_and_merged_are_bold() {
        assert!(Glyph::Error.bold() && Glyph::Merged.bold());
        assert!(!Glyph::Alive.bold() && !Glyph::Dead.bold());
    }
}
