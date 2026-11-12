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
