//! File names: the one an upload is saved and shown under, and the `content-disposition`
//! a download sends.

const FALLBACK: &str = "file";
/// Bytes, extension included; the store allows 200, the UI shows less.
const MAX_NAME_BYTES: usize = 120;
const MAX_EXTENSION: usize = 10;

/// Invisible characters that can disguise a name, like a right-to-left override.
fn is_hidden(c: char) -> bool {
    c.is_control()
        || matches!(c, '\u{200b}'..='\u{200f}' | '\u{202a}'..='\u{202e}')
        || matches!(c, '\u{2060}'..='\u{2064}' | '\u{2066}'..='\u{2069}' | '\u{feff}')
}

fn is_edge(c: char) -> bool {
    c == '.' || c.is_whitespace()
}

/// The longest start of `text` that fits in `max` bytes.
fn cut(text: &str, max: usize) -> &str {
    let end = (0..=max.min(text.len()))
        .rev()
        .find(|i| text.is_char_boundary(*i))
        .unwrap_or(0);
    &text[..end]
}

/// The stem and the extension as written, when that is 1 to 10 ASCII letters or digits.
fn split(name: &str) -> (&str, Option<&str>) {
    let safe = |e: &str| e.len() <= MAX_EXTENSION && e.chars().all(|c| c.is_ascii_alphanumeric());
    match name.rsplit_once('.') {
        Some((stem, ext)) if !stem.is_empty() && !ext.is_empty() && safe(ext) => (stem, Some(ext)),
        _ => (name, None),
    }
}

/// The last path part, without invisible characters, edge dots or spaces, at most 120
/// bytes with its extension kept. A sniffed image gets an extension of its type.
pub fn file_name(raw: &str, image: Option<&[&str]>) -> String {
    let last = raw.rsplit(['/', '\\']).next().unwrap_or_default();
    let visible: String = last.chars().filter(|c| !is_hidden(*c)).collect();
    let (stem, found) = split(visible.trim_matches(is_edge));
    let matches =
        |exts: &[&str]| found.is_some_and(|f| exts.contains(&f.to_ascii_lowercase().as_str()));
    let ext = match image {
        Some(exts) if !matches(exts) => exts.first().copied(),
        _ => found,
    };
    let room = MAX_NAME_BYTES - ext.map_or(0, |e| e.len() + 1);
    let stem = cut(stem, room).trim_end_matches(is_edge);
    let stem = if stem.is_empty() { FALLBACK } else { stem };
    match ext {
        Some(ext) => format!("{stem}.{ext}"),
        None => stem.to_string(),
    }
}

/// RFC 5987 `attr-char`: what may stand unescaped in `filename*`.
fn is_attr_char(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b"!#$&+-.^_`|~".contains(&b)
}

/// `inline` or `attachment`, an ASCII `filename` and the exact `filename*`.
pub fn disposition(inline: bool, name: &str) -> String {
    let kind = if inline { "inline" } else { "attachment" };
    let plain: String = name
        .chars()
        .map(|c| match c {
            ' ' => ' ',
            '"' | '\\' | '%' => '_',
            c if c.is_ascii_graphic() => c,
            _ => '_',
        })
        .collect();
    let encoded: String = name
        .bytes()
        .map(|b| match is_attr_char(b) {
            true => char::from(b).to_string(),
            false => format!("%{b:02X}"),
        })
        .collect();
    format!("{kind}; filename=\"{plain}\"; filename*=UTF-8''{encoded}")
}

#[cfg(test)]
mod tests {
    use super::*;

    const PNG: Option<&[&str]> = Some(&["png"]);
    const JPEG: Option<&[&str]> = Some(&["jpg", "jpeg"]);

    #[test]
    fn names_lose_paths_and_invisible_characters() {
        assert_eq!(file_name("../../etc/passwd", None), "passwd");
        assert_eq!(file_name("C:\\x\\shot.png", None), "shot.png");
        assert_eq!(file_name("a\0b\n\r\t.txt", None), "ab.txt");
        assert_eq!(file_name("invoice\u{202e}fdp.exe", None), "invoicefdp.exe");
        assert_eq!(file_name("..", None), "file");
        assert_eq!(file_name("", None), "file");
        assert_eq!(file_name(" .env ", None), "env");
        assert_eq!(file_name("report.final draft", None), "report.final draft");
    }

    #[test]
    fn long_names_keep_their_extension() {
        let long = format!("{}.markdown", "é".repeat(100));
        let name = file_name(&long, None);
        assert!(
            name.len() <= MAX_NAME_BYTES && name.ends_with("é.markdown"),
            "{name}"
        );
        let odd = format!("{}.{}", "a".repeat(200), "x".repeat(11));
        assert_eq!(file_name(&odd, None).len(), MAX_NAME_BYTES);
    }

    #[test]
    fn images_get_an_extension_of_their_type() {
        assert_eq!(file_name("shot", PNG), "shot.png");
        assert_eq!(file_name("shot.PNG", PNG), "shot.PNG");
        assert_eq!(file_name("shot.txt", PNG), "shot.png");
        assert_eq!(file_name("photo.jpeg", JPEG), "photo.jpeg");
        assert_eq!(file_name("photo.gif", JPEG), "photo.jpg");
        assert_eq!(file_name("../", PNG), "file.png");
    }

    #[test]
    fn dispositions_are_ascii() {
        assert_eq!(
            disposition(true, "a b.png"),
            "inline; filename=\"a b.png\"; filename*=UTF-8''a%20b.png"
        );
        assert_eq!(
            disposition(false, "\"é\\%.txt"),
            "attachment; filename=\"____.txt\"; filename*=UTF-8''%22%C3%A9%5C%25.txt"
        );
    }
}
