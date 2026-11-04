use serde::de::DeserializeOwned;

use super::error::{DefKind, DefinitionError};

/// A parsed definition and the TOML it came from, kept as written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Parsed<T> {
    pub source: String,
    pub def: T,
}

/// Parse TOML into `T`. Unknown fields and bad values fail here, one error at a time.
pub(crate) fn parse<T: DeserializeOwned>(
    kind: DefKind,
    source: &str,
) -> Result<Parsed<T>, DefinitionError> {
    toml::from_str::<T>(source)
        .map(|def| Parsed {
            source: source.to_string(),
            def,
        })
        .map_err(|e| DefinitionError {
            kind,
            name: name_in(source),
            step: None,
            field: None,
            message: e.message().trim().to_string(),
            line: e.span().map(|span| line_of(source, span.start)),
        })
}

/// The top level `name`, if the source is valid TOML and has one.
fn name_in(source: &str) -> Option<String> {
    let table: toml::Table = toml::from_str(source).ok()?;
    table.get("name")?.as_str().map(str::to_string)
}

fn line_of(source: &str, offset: usize) -> usize {
    let end = offset.min(source.len());
    source.as_bytes()[..end]
        .iter()
        .filter(|b| **b == b'\n')
        .count()
        + 1
}
