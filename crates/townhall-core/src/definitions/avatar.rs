//! The `[avatar]` table of an agent type: the parts of its pixel face, and optionally the
//! face as a literal 9x9 grid. Every key is optional; a missing key (or table) falls back to
//! the face the web app derives from the type's name.

use serde::{Deserialize, Serialize};

pub const GRID_SIZE: usize = 9;
pub const GRID_CELLS: &str = ".#o";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Head {
    Square,
    Round,
    Domed,
    Tapered,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Eyes {
    Dots,
    Tall,
    Wide,
    Visor,
    Cyclops,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Mouth {
    Flat,
    Smile,
    Open,
    Grill,
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Accessory {
    None,
    Antenna,
    Twin,
    Horns,
    Crest,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AvatarColor {
    Accent,
    Ok,
    Info,
    Fg,
    FgMuted,
}

/// `[avatar]` in an agent type's TOML. `grid`, when set, is the picture: nine rows of nine
/// cells, `#` body, `o` a face cut-out, `.` empty. The part keys then only tint it (`color`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AvatarDef {
    #[serde(default)]
    pub head: Option<Head>,
    #[serde(default)]
    pub eyes: Option<Eyes>,
    #[serde(default)]
    pub mouth: Option<Mouth>,
    #[serde(default)]
    pub accessory: Option<Accessory>,
    #[serde(default)]
    pub color: Option<AvatarColor>,
    #[serde(default)]
    pub grid: Option<Vec<String>>,
}

impl AvatarDef {
    /// Every problem in the grid, as the text that follows `avatar.grid`.
    pub fn grid_problems(&self) -> Vec<String> {
        let Some(rows) = &self.grid else {
            return Vec::new();
        };
        let height = (rows.len() != GRID_SIZE)
            .then(|| format!("must have {GRID_SIZE} rows, found {}", rows.len()));
        let cells = rows.iter().enumerate().filter_map(|(at, row)| {
            let width = row.chars().count();
            let stray = row.chars().find(|c| !GRID_CELLS.contains(*c));
            match (width != GRID_SIZE, stray) {
                (true, _) => Some(format!(
                    "row {} must have {GRID_SIZE} cells, found {width}",
                    at + 1
                )),
                (false, Some(c)) => Some(format!(
                    "row {} has `{}`: use only `.` (empty), `#` (body) and `o` (face)",
                    at + 1,
                    c.escape_debug()
                )),
                (false, None) => None,
            }
        });
        height.into_iter().chain(cells).collect()
    }
}
