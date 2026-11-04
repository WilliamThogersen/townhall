//! An upload's type. The bytes decide whether it is an image (png, jpeg, gif, webp by
//! their first bytes); otherwise the part's content type, then the name's extension. An
//! image type the bytes do not back up (an svg, a text file sent as `image/png`) is never
//! kept, so only real raster images are `kind: image` and served inline.

pub const UNKNOWN: &str = "application/octet-stream";
const MAX_MIME_LEN: usize = 100;

/// Each image type with the extension it is saved under, then the other names it takes.
const IMAGES: [(&str, &[&str]); 4] = [
    ("image/png", &["png"]),
    ("image/jpeg", &["jpg", "jpeg"]),
    ("image/gif", &["gif"]),
    ("image/webp", &["webp"]),
];

const BY_EXTENSION: [(&str, &str); 12] = [
    ("txt", "text/plain"),
    ("log", "text/plain"),
    ("md", "text/markdown"),
    ("csv", "text/csv"),
    ("json", "application/json"),
    ("pdf", "application/pdf"),
    ("zip", "application/zip"),
    ("html", "text/html"),
    ("xml", "application/xml"),
    ("yaml", "application/yaml"),
    ("yml", "application/yaml"),
    ("toml", "application/toml"),
];

/// The image type the bytes start with, if any.
pub fn sniff(bytes: &[u8]) -> Option<&'static str> {
    let webp = bytes.len() >= 12 && bytes.starts_with(b"RIFF") && &bytes[8..12] == b"WEBP";
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        Some("image/png")
    } else if bytes.starts_with(&[0xff, 0xd8, 0xff]) {
        Some("image/jpeg")
    } else if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        Some("image/gif")
    } else {
        webp.then_some("image/webp")
    }
}

/// A content type worth keeping: `type/subtype` (parameters allowed), printable ASCII.
fn clean(given: &str) -> Option<String> {
    let given = given.trim().to_ascii_lowercase();
    let essence = given.split(';').next().unwrap_or_default().trim();
    let ok = given.len() <= MAX_MIME_LEN
        && essence.split('/').count() == 2
        && !essence.starts_with('/')
        && !essence.ends_with('/')
        && given.chars().all(|c| c.is_ascii_graphic() || c == ' ');
    (ok && essence != UNKNOWN).then_some(given)
}

fn extension(name: &str) -> Option<String> {
    let (stem, ext) = name.rsplit_once('.')?;
    (!stem.is_empty() && !ext.is_empty()).then(|| ext.to_ascii_lowercase())
}

fn by_extension(name: &str) -> Option<String> {
    let ext = extension(name)?;
    let found = BY_EXTENSION.iter().find(|(e, _)| *e == ext);
    found.map(|(_, mime)| mime.to_string())
}

/// The upload's mime, see the module doc.
pub fn detect(bytes: &[u8], given: Option<&str>, name: &str) -> String {
    if let Some(image) = sniff(bytes) {
        return image.to_string();
    }
    let not_image = |mime: &String| !mime.starts_with("image/");
    given
        .and_then(clean)
        .filter(not_image)
        .or_else(|| by_extension(name))
        .unwrap_or_else(|| UNKNOWN.to_string())
}

/// The extensions of a sniffed image type, the one it is saved under first; `None` for
/// anything else.
pub fn image_extensions(mime: &str) -> Option<&'static [&'static str]> {
    IMAGES
        .iter()
        .find(|(m, _)| *m == mime)
        .map(|(_, exts)| *exts)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn images_are_told_by_their_first_bytes() {
        assert_eq!(sniff(b"\x89PNG\r\n\x1a\nrest"), Some("image/png"));
        assert_eq!(sniff(b"\xff\xd8\xff\xdb"), Some("image/jpeg"));
        assert_eq!(sniff(b"GIF87a.."), Some("image/gif"));
        assert_eq!(sniff(b"RIFF\0\0\0\0WEBPVP8 "), Some("image/webp"));
        assert_eq!(sniff(b"RIFF\0\0\0\0WAVEfmt "), None);
        assert_eq!(sniff(b"RIFF"), None);
        assert_eq!(sniff(b""), None);
    }

    #[test]
    fn claims_fall_back_and_unbacked_images_are_unknown() {
        assert_eq!(
            detect(b"x", Some("Text/Plain; charset=UTF-8"), "a"),
            "text/plain; charset=utf-8"
        );
        assert_eq!(
            detect(b"x", Some("application/octet-stream"), "a.md"),
            "text/markdown"
        );
        assert_eq!(detect(b"x", Some("nonsense"), "a.csv"), "text/csv");
        assert_eq!(detect(b"x", Some("a/b/c"), "a"), UNKNOWN);
        assert_eq!(detect(b"x", Some("te\u{7f}xt/plain"), "a"), UNKNOWN);
        assert_eq!(
            detect(b"x", Some(&format!("text/{}", "x".repeat(100))), "a"),
            UNKNOWN
        );
        assert_eq!(detect(b"x", None, ".json"), UNKNOWN);
        assert_eq!(detect(b"x", Some("image/png"), "a.txt"), "text/plain");
        assert_eq!(detect(b"x", Some("image/svg+xml"), "a.png"), UNKNOWN);
        assert_eq!(detect(b"x", None, "a.svg"), UNKNOWN);
        assert_eq!(detect(b"x", None, "a.png"), UNKNOWN);
        assert_eq!(detect(b"GIF89a", Some("text/plain"), "a.txt"), "image/gif");
    }

    #[test]
    fn image_types_have_their_extensions() {
        assert_eq!(image_extensions("image/jpeg"), Some(&["jpg", "jpeg"][..]));
        assert_eq!(image_extensions("image/svg+xml"), None);
        assert_eq!(image_extensions("text/plain"), None);
        assert_eq!(extension("a.TAR.GZ").as_deref(), Some("gz"));
        assert_eq!(extension(".bashrc"), None);
        assert_eq!(extension("a."), None);
    }
}
