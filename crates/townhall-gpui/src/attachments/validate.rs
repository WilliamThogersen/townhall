use super::format::file_extension;

pub const MAX_ATTACHMENT_BYTES: u64 = 20 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileMeta {
    pub name: String,
    pub mime: String,
    pub size: u64,
}

const IMAGE_TYPES: [&str; 4] = ["image/png", "image/jpeg", "image/gif", "image/webp"];

const DOCUMENT_TYPES: &[&str] = &[
    "application/pdf",
    "application/json",
    "application/xml",
    "application/yaml",
    "application/x-yaml",
    "application/toml",
    "application/javascript",
    "application/typescript",
    "application/sql",
    "application/x-sh",
];

const CODE_EXTENSIONS: &[&str] = &[
    "txt", "md", "markdown", "log", "csv", "tsv", "json", "jsonl", "yaml", "yml", "toml", "xml",
    "ini", "conf", "cfg", "har", "diff", "patch", "pdf", "ts", "tsx", "js", "jsx", "mjs", "cjs",
    "css", "scss", "less", "html", "htm", "vue", "svelte", "svg", "rs", "go", "py", "rb", "php",
    "java", "kt", "kts", "swift", "c", "h", "cc", "cpp", "hpp", "cs", "m", "scala", "lua", "pl",
    "r", "dart", "ex", "exs", "erl", "hs", "clj", "elm", "zig", "sh", "bash", "zsh", "fish", "ps1",
    "sql", "graphql", "gql", "proto", "tf", "hcl", "gradle", "lock", "twig", "blade", "liquid",
    "env",
];

const CODE_NAMES: &[&str] = &[
    "dockerfile",
    "makefile",
    "justfile",
    "procfile",
    "gemfile",
    "rakefile",
    "brewfile",
    "jenkinsfile",
    "vagrantfile",
    "license",
    "readme",
    "changelog",
];

pub fn is_image(mime: &str) -> bool {
    IMAGE_TYPES.contains(&mime)
}

pub fn mime_for(name: &str) -> String {
    let mime = match file_extension(name).as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "pdf" => "application/pdf",
        "json" => "application/json",
        "txt" | "log" => "text/plain",
        "md" | "markdown" => "text/markdown",
        "csv" => "text/csv",
        _ => "application/octet-stream",
    };
    mime.to_string()
}

fn is_accepted_type(file: &FileMeta) -> bool {
    if is_image(&file.mime) || file.mime.starts_with("text/") {
        return true;
    }
    if DOCUMENT_TYPES.contains(&file.mime.as_str()) {
        return true;
    }
    let ext = file_extension(&file.name);
    if !ext.is_empty() {
        return CODE_EXTENSIONS.contains(&ext.as_str());
    }
    CODE_NAMES.contains(&file.name.to_lowercase().as_str())
}

pub fn reject_reason(file: &FileMeta) -> Option<String> {
    if file.size > MAX_ATTACHMENT_BYTES {
        return Some(format!("{} is over 20 MB", file.name));
    }
    if file.size == 0 {
        return Some(format!("{} is empty", file.name));
    }
    if !is_accepted_type(file) {
        return Some(format!("{} is not an image, text, code or pdf", file.name));
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(name: &str, mime: &str, size: u64) -> FileMeta {
        FileMeta {
            name: name.into(),
            mime: mime.into(),
            size,
        }
    }

    #[test]
    fn four_raster_types_are_images_and_everything_else_is_a_file() {
        for mime in ["image/png", "image/jpeg", "image/gif", "image/webp"] {
            assert!(is_image(mime), "{mime}");
        }
        for mime in ["image/svg+xml", "image/heic", "text/plain", ""] {
            assert!(!is_image(mime), "{mime:?}");
        }
    }

    #[test]
    fn accepted_files_have_no_reason() {
        for (name, mime) in [
            ("screenshot.png", "image/png"),
            ("notes.txt", "text/plain"),
            ("auth-flow.pdf", "application/pdf"),
            ("package.json", "application/json"),
            ("query.ts", "video/mp2t"),
            ("main.rs", ""),
            ("localhost.har", ""),
            ("Dockerfile", ""),
            ("Makefile", "application/octet-stream"),
        ] {
            assert_eq!(reject_reason(&file(name, mime, 10)), None, "{name}");
        }
    }

    #[test]
    fn other_types_are_refused_as_not_an_image_text_code_or_pdf() {
        for (name, mime) in [
            ("movie.mp4", "video/mp4"),
            ("photo.heic", "image/heic"),
            ("installer.dmg", "application/x-apple-diskimage"),
            ("mystery", ""),
        ] {
            assert_eq!(
                reject_reason(&file(name, mime, 10)),
                Some(format!("{name} is not an image, text, code or pdf"))
            );
        }
    }

    #[test]
    fn a_file_over_20_mb_is_refused_before_its_type() {
        assert_eq!(
            reject_reason(&file(
                "screenshot-3.png",
                "image/png",
                MAX_ATTACHMENT_BYTES + 1
            )),
            Some("screenshot-3.png is over 20 MB".into())
        );
        assert_eq!(
            reject_reason(&file("exact.png", "image/png", MAX_ATTACHMENT_BYTES)),
            None
        );
    }

    #[test]
    fn an_empty_file_is_refused() {
        assert_eq!(
            reject_reason(&file("blank.txt", "text/plain", 0)),
            Some("blank.txt is empty".into())
        );
    }

    #[test]
    fn a_mime_is_guessed_from_the_extension_for_files_from_disk() {
        assert_eq!(mime_for("a.PNG"), "image/png");
        assert_eq!(mime_for("a.jpg"), "image/jpeg");
        assert_eq!(mime_for("a.jpeg"), "image/jpeg");
        assert_eq!(mime_for("a.gif"), "image/gif");
        assert_eq!(mime_for("a.webp"), "image/webp");
        assert_eq!(mime_for("a.pdf"), "application/pdf");
        assert_eq!(mime_for("a.json"), "application/json");
        assert_eq!(mime_for("a.txt"), "text/plain");
        assert_eq!(mime_for("a.md"), "text/markdown");
        assert_eq!(mime_for("a.har"), "application/octet-stream");
    }
}
