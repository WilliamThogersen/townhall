use gpui::{rgba, App, FontFeatures, Global, Rgba, WindowAppearance};

pub const FONT: &str = "JetBrains Mono";

pub const TEXT_XS: f32 = 11.0;
pub const TEXT_SM: f32 = 12.0;
pub const TEXT_MD: f32 = 13.0;
pub const TEXT_LG: f32 = 15.0;
pub const TEXT_XL: f32 = 20.0;
pub const LEADING_XL: f32 = 28.0;
pub const HOME: f32 = 720.0;
pub const LEADING_SM: f32 = 18.0;
pub const LEADING_MD: f32 = 20.0;
pub const LEADING_LG: f32 = 22.0;
pub const SPACE_1: f32 = 4.0;
pub const SPACE_2: f32 = 8.0;
pub const SPACE_3: f32 = 12.0;
pub const SPACE_4: f32 = 16.0;
pub const SPACE_5: f32 = 24.0;
pub const SPACE_6: f32 = 32.0;
pub const CONTROL_SM: f32 = 24.0;
pub const RAIL: f32 = 320.0;
pub const ROW: f32 = 28.0;
pub const BAR: f32 = 36.0;
pub const KEYBAR: f32 = 24.0;
pub const SIDEBAR: f32 = 272.0;
pub const BORDER: f32 = 1.0;
pub const ACCENT_RULE: f32 = 2.0;
pub const BUBBLE: f32 = 0.85;

#[cfg(test)]
const SIZES: &[(&str, f32)] = &[
    ("--th-text-xs", TEXT_XS),
    ("--th-text-sm", TEXT_SM),
    ("--th-text-md", TEXT_MD),
    ("--th-text-lg", TEXT_LG),
    ("--th-text-xl", TEXT_XL),
    ("--th-leading-xl", LEADING_XL),
    ("--th-home", HOME),
    ("--th-leading-sm", LEADING_SM),
    ("--th-leading-md", LEADING_MD),
    ("--th-leading-lg", LEADING_LG),
    ("--th-space-1", SPACE_1),
    ("--th-space-2", SPACE_2),
    ("--th-space-3", SPACE_3),
    ("--th-space-4", SPACE_4),
    ("--th-space-5", SPACE_5),
    ("--th-space-6", SPACE_6),
    ("--th-control-sm", CONTROL_SM),
    ("--th-rail", RAIL),
    ("--th-row", ROW),
    ("--th-bar", BAR),
    ("--th-keybar", KEYBAR),
    ("--th-sidebar", SIDEBAR),
    ("--th-border", BORDER),
];

const MEASURE_MIN_CH: f32 = 110.0;
const MEASURE_MAX_CH: f32 = 150.0;
const MEASURE_SHARE: f32 = 0.66;
const GLYPH_ADVANCE_EM: f32 = 0.6;

pub fn measure(pane_width: f32) -> f32 {
    let ch = TEXT_MD * GLYPH_ADVANCE_EM;
    (MEASURE_MIN_CH * ch)
        .max(pane_width * MEASURE_SHARE)
        .min(MEASURE_MAX_CH * ch)
}

macro_rules! palette {
    ($($field:ident => $css:literal, $dark:literal, $light:literal;)*) => {
        #[derive(Debug, Clone, Copy, PartialEq)]
        pub struct Palette {
            $(pub $field: Rgba,)*
        }

        impl Palette {
            pub fn dark() -> Self {
                Self { $($field: rgba($dark),)* }
            }

            pub fn light() -> Self {
                Self { $($field: rgba($light),)* }
            }

            #[cfg(test)]
            pub fn entries(&self) -> Vec<(&'static str, u32)> {
                vec![$(($css, pack(self.$field)),)*]
            }
        }
    };
}

#[cfg(test)]
fn pack(c: Rgba) -> u32 {
    let byte = |v: f32| (v * 255.0).round() as u32;
    byte(c.r) << 24 | byte(c.g) << 16 | byte(c.b) << 8 | byte(c.a)
}

palette! {
    bg => "--th-bg", 0x0b0b0cff, 0xf4f3efff;
    surface => "--th-surface", 0x111113ff, 0xfbfbf8ff;
    surface_2 => "--th-surface-2", 0x18181bff, 0xecebe6ff;
    surface_3 => "--th-surface-3", 0x212125ff, 0xe2e1dbff;
    line => "--th-line", 0x26262aff, 0xdeddd6ff;
    line_strong => "--th-line-strong", 0x38383eff, 0xc9c7bfff;
    line_control => "--th-line-control", 0x6a6a72ff, 0x8c8a82ff;
    fg => "--th-fg", 0xe8e6e1ff, 0x191917ff;
    fg_muted => "--th-fg-muted", 0xa8a6a0ff, 0x52514cff;
    fg_faint => "--th-fg-faint", 0x8a8882ff, 0x6b6a64ff;
    on_inverse => "--th-on-inverse", 0x0b0b0cff, 0xf4f3efff;
    accent => "--th-accent", 0xffb238ff, 0x955700ff;
    accent_strong => "--th-accent-strong", 0xffc870ff, 0x7a4700ff;
    on_accent => "--th-on-accent", 0x0b0b0cff, 0xffffffff;
    accent_tint => "--th-accent-tint", 0xffb2381a, 0xc47a001f;
    ok => "--th-ok", 0x5fd38dff, 0x1b7543ff;
    ok_tint => "--th-ok-tint", 0x5fd38d1a, 0x1b75431a;
    danger => "--th-danger", 0xff6b6bff, 0xc02b2bff;
    danger_strong => "--th-danger-strong", 0xff9090ff, 0x9e2020ff;
    on_danger => "--th-on-danger", 0x0b0b0cff, 0xffffffff;
    danger_tint => "--th-danger-tint", 0xff6b6b1a, 0xc02b2b14;
    info => "--th-info", 0x6cb6ffff, 0x1c5cbbff;
    info_tint => "--th-info-tint", 0x6cb6ff1a, 0x1c5cbb14;
    selection => "--th-selection", 0xffb23847, 0xc47a0038;
    backdrop => "--th-backdrop", 0x0000009e, 0x19191766;
    lightbox_bg => "--th-lightbox-bg", 0x080809f7, 0xf4f3eff7;
    heat_0 => "--th-heat-0", 0x18181bff, 0xe5e4deff;
    heat_1 => "--th-heat-1", 0x1c4a2fff, 0xb5e0c2ff;
    heat_2 => "--th-heat-2", 0x287a4aff, 0x74c290ff;
    heat_3 => "--th-heat-3", 0x42ad6bff, 0x36975aff;
    heat_4 => "--th-heat-4", 0x72e6a0ff, 0x1a663aff;
    term_bg => "--th-term-bg", 0x0b0b0cff, 0xfbfbf8ff;
    term_fg => "--th-term-fg", 0xe8e6e1ff, 0x191917ff;
    term_cursor => "--th-term-cursor", 0xffb238ff, 0x955700ff;
    term_selection => "--th-term-selection", 0xffb23847, 0xc47a0038;
    ansi_black => "--th-ansi-black", 0x26262aff, 0x191917ff;
    ansi_red => "--th-ansi-red", 0xff6b6bff, 0xc02b2bff;
    ansi_green => "--th-ansi-green", 0x5fd38dff, 0x1b7543ff;
    ansi_yellow => "--th-ansi-yellow", 0xffb238ff, 0x955700ff;
    ansi_blue => "--th-ansi-blue", 0x6cb6ffff, 0x1c5cbbff;
    ansi_magenta => "--th-ansi-magenta", 0xd79bffff, 0x8a3db5ff;
    ansi_cyan => "--th-ansi-cyan", 0x5fd7d7ff, 0x0e727cff;
    ansi_white => "--th-ansi-white", 0xc8c6c0ff, 0x6b6a64ff;
    ansi_bright_black => "--th-ansi-bright-black", 0x6a6a72ff, 0x52514cff;
    ansi_bright_red => "--th-ansi-bright-red", 0xff9090ff, 0x9e2020ff;
    ansi_bright_green => "--th-ansi-bright-green", 0x8ae6adff, 0x145c34ff;
    ansi_bright_yellow => "--th-ansi-bright-yellow", 0xffc870ff, 0x7a4700ff;
    ansi_bright_blue => "--th-ansi-bright-blue", 0x9acdffff, 0x164a98ff;
    ansi_bright_magenta => "--th-ansi-bright-magenta", 0xe6bdffff, 0x702f94ff;
    ansi_bright_cyan => "--th-ansi-bright-cyan", 0x8ae6e6ff, 0x0a5a62ff;
    ansi_bright_white => "--th-ansi-bright-white", 0xf6f4efff, 0x3a3a36ff;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Dark,
    Light,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Theme {
    pub mode: Mode,
    pub palette: Palette,
}

impl Global for Theme {}

impl Theme {
    pub fn new(mode: Mode) -> Self {
        let palette = match mode {
            Mode::Dark => Palette::dark(),
            Mode::Light => Palette::light(),
        };
        Self { mode, palette }
    }

    pub fn for_appearance(appearance: WindowAppearance) -> Self {
        match appearance {
            WindowAppearance::Light | WindowAppearance::VibrantLight => Self::new(Mode::Light),
            WindowAppearance::Dark | WindowAppearance::VibrantDark => Self::new(Mode::Dark),
        }
    }

    pub fn of(cx: &App) -> Self {
        *cx.global::<Theme>()
    }

    pub fn status_alive(&self) -> Rgba {
        self.palette.ok
    }

    pub fn status_working(&self) -> Rgba {
        self.palette.info
    }

    pub fn status_waiting(&self) -> Rgba {
        self.palette.accent
    }

    pub fn status_error(&self) -> Rgba {
        self.palette.danger
    }

    pub fn status_dead(&self) -> Rgba {
        self.palette.fg_faint
    }
}

const CARET_INTERVAL: std::time::Duration = std::time::Duration::from_millis(500);
const CARET_IDLE: std::time::Duration = std::time::Duration::from_secs(3);

pub fn apply_to_components(theme: &Theme, cx: &mut App) {
    let p = theme.palette;
    let component = gpui_component::Theme::global_mut(cx);
    component.colors.foreground = p.fg.into();
    component.colors.muted_foreground = p.fg_faint.into();
    component.colors.caret = p.accent.into();
    component.colors.selection = p.selection.into();
    component.colors.input = p.line_control.into();
    component.colors.background = p.surface.into();
    component.font_family = FONT.into();
    component.font_size = gpui::px(TEXT_MD);
    component.radius = gpui::px(0.0);
    component.radius_lg = gpui::px(0.0);
    component.cursor_blink =
        gpui_component::input::CursorBlink::blinking(CARET_INTERVAL).with_idle_timeout(CARET_IDLE);
    gpui_component::Theme::sync_base(cx);
}

pub fn no_ligatures() -> FontFeatures {
    FontFeatures::disable_ligatures()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    const TOKENS: &str = include_str!("../../../docs/style-guide/tokens.css");

    fn block<'a>(css: &'a str, opener: &str) -> &'a str {
        let start = css
            .find(opener)
            .unwrap_or_else(|| panic!("no {opener} block"));
        let body = &css[start + opener.len()..];
        &body[..body.find("\n}").expect("unterminated block")]
    }

    fn parse_color(value: &str) -> Option<u32> {
        let value = value.trim();
        if let Some(hex) = value.strip_prefix('#') {
            let rgb = u32::from_str_radix(hex, 16).ok()?;
            return match hex.len() {
                6 => Some(rgb << 8 | 0xff),
                _ => None,
            };
        }
        let args = value.strip_prefix("rgba(")?.strip_suffix(')')?;
        let parts: Vec<&str> = args.split(',').map(str::trim).collect();
        let [r, g, b, a] = parts.as_slice() else {
            return None;
        };
        let channel = |s: &str| s.parse::<u32>().unwrap();
        let alpha = (a.parse::<f32>().unwrap() * 255.0).round() as u32;
        Some(channel(r) << 24 | channel(g) << 16 | channel(b) << 8 | alpha)
    }

    fn declarations(block: &str) -> BTreeMap<String, String> {
        block
            .lines()
            .filter_map(|line| {
                let line = line.trim();
                let (name, rest) = line.split_once(':')?;
                name.starts_with("--th-").then(|| {
                    (
                        name.to_string(),
                        rest.trim_end_matches(';').trim().to_string(),
                    )
                })
            })
            .collect()
    }

    fn colors(block: &str) -> BTreeMap<String, u32> {
        declarations(block)
            .into_iter()
            .filter_map(|(name, value)| parse_color(&value).map(|c| (name, c)))
            .collect()
    }

    fn assert_matches_css(palette: Palette, css: BTreeMap<String, u32>) {
        let ours: BTreeMap<String, u32> = palette
            .entries()
            .into_iter()
            .map(|(name, value)| (name.to_string(), value))
            .collect();
        for (name, value) in &css {
            match ours.get(name) {
                Some(mine) => assert_eq!(
                    mine, value,
                    "{name}: theme has {mine:#010x}, tokens.css has {value:#010x}"
                ),
                None => panic!("{name} is in tokens.css but not in the Rust palette"),
            }
        }
        for name in ours.keys() {
            assert!(css.contains_key(name), "{name} is not in tokens.css");
        }
    }

    #[test]
    fn dark_palette_equals_tokens_css_root() {
        assert_matches_css(Palette::dark(), colors(block(TOKENS, ":root {")));
    }

    #[test]
    fn light_palette_equals_tokens_css_light_overrides_over_root() {
        let mut css = colors(block(TOKENS, ":root {"));
        css.extend(colors(block(TOKENS, ":root[data-theme=\"light\"] {")));
        assert_matches_css(Palette::light(), css);
    }

    #[test]
    fn sizes_equal_tokens_css() {
        let root = declarations(block(TOKENS, ":root {"));
        for (name, px) in SIZES {
            let value = root
                .get(*name)
                .unwrap_or_else(|| panic!("{name} is not in tokens.css"));
            assert_eq!(
                value.trim_end_matches("px").parse::<f32>().unwrap(),
                *px,
                "{name}"
            );
        }
    }

    #[test]
    fn measure_follows_the_css_formula() {
        let ch = 13.0 * 0.6;
        assert_eq!(measure(400.0), 110.0 * ch);
        assert_eq!(measure(1600.0), 1600.0 * 0.66);
        assert_eq!(measure(4000.0), 150.0 * ch);
    }

    #[test]
    fn status_colors_alias_the_css_status_tokens() {
        let root = declarations(block(TOKENS, ":root {"));
        let t = Theme::new(Mode::Dark);
        let pairs = [
            ("--th-status-alive", "--th-ok", t.status_alive()),
            ("--th-status-working", "--th-info", t.status_working()),
            ("--th-status-waiting", "--th-accent", t.status_waiting()),
            ("--th-status-error", "--th-danger", t.status_error()),
            ("--th-status-dead", "--th-fg-faint", t.status_dead()),
        ];
        for (status, target, actual) in pairs {
            assert_eq!(root[status], format!("var({target})"), "{status}");
            let wanted = t
                .palette
                .entries()
                .into_iter()
                .find(|(name, _)| *name == target)
                .unwrap()
                .1;
            assert_eq!(pack(actual), wanted, "{status}");
        }
    }

    #[test]
    fn appearance_picks_the_mode() {
        assert_eq!(
            Theme::for_appearance(WindowAppearance::Dark).mode,
            Mode::Dark
        );
        assert_eq!(
            Theme::for_appearance(WindowAppearance::VibrantDark).mode,
            Mode::Dark
        );
        assert_eq!(
            Theme::for_appearance(WindowAppearance::Light).mode,
            Mode::Light
        );
        assert_eq!(
            Theme::for_appearance(WindowAppearance::VibrantLight).mode,
            Mode::Light
        );
    }

    #[test]
    fn ligatures_are_off() {
        assert_eq!(no_ligatures().is_calt_enabled(), Some(false));
    }

    #[gpui::test]
    fn the_caret_stops_blinking_when_idle_so_the_app_can_sleep(cx: &mut gpui::TestAppContext) {
        cx.update(|cx| {
            gpui_component::init(cx);
            apply_to_components(&Theme::new(Mode::Dark), cx);
            let blink = gpui_component::Theme::global(cx).cursor_blink;
            assert_eq!(blink.interval(), Some(CARET_INTERVAL));
            assert_eq!(blink.idle_timeout(), Some(CARET_IDLE));
        });
    }

    #[gpui::test]
    fn components_use_the_palette_and_square_corners(cx: &mut gpui::TestAppContext) {
        cx.update(|cx| {
            gpui_component::init(cx);
            let theme = Theme::new(Mode::Dark);
            apply_to_components(&theme, cx);
            let component = gpui_component::Theme::global(cx);
            assert_eq!(component.colors.caret, theme.palette.accent.into());
            assert_eq!(component.colors.foreground, theme.palette.fg.into());
            assert_eq!(component.font_family.as_ref(), FONT);
            assert_eq!(component.radius, gpui::px(0.0));
        });
    }
}
