use gpui::{rgb, rgba, Rgba};

pub const FONT: &str = "Menlo";

pub const BAR: f32 = 36.0;
pub const SIDEBAR: f32 = 272.0;
pub const KEYBAR: f32 = 24.0;
pub const MEASURE: f32 = 600.0;

pub fn bg() -> Rgba {
    rgb(0x0b0b0c)
}
pub fn surface() -> Rgba {
    rgb(0x111113)
}
pub fn surface_2() -> Rgba {
    rgb(0x18181b)
}
pub fn line() -> Rgba {
    rgb(0x26262a)
}
pub fn fg() -> Rgba {
    rgb(0xe8e6e1)
}
pub fn fg_muted() -> Rgba {
    rgb(0xa8a6a0)
}
pub fn fg_faint() -> Rgba {
    rgb(0x8a8882)
}
pub fn accent() -> Rgba {
    rgb(0xffb238)
}
pub fn accent_tint() -> Rgba {
    rgba(0xffb2381a)
}
pub fn ok() -> Rgba {
    rgb(0x5fd38d)
}
pub fn danger() -> Rgba {
    rgb(0xff6b6b)
}
pub fn info() -> Rgba {
    rgb(0x6cb6ff)
}
pub fn info_tint() -> Rgba {
    rgba(0x6cb6ff1a)
}
