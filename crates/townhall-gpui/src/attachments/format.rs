const KILO: f64 = 1024.0;
const UNITS: [&str; 3] = ["KB", "MB", "GB"];
const TAG_LETTERS: usize = 4;

fn compact(value: f64) -> String {
    if value >= 10.0 {
        return format!("{}", value.round() as u64);
    }
    let rounded = (value * 10.0).round() / 10.0;
    if rounded.fract() == 0.0 {
        format!("{}", rounded as u64)
    } else {
        format!("{rounded:.1}")
    }
}

pub fn format_size(bytes: u64) -> String {
    if (bytes as f64) < KILO {
        return format!("{bytes} B");
    }
    let step = (((bytes as f64).ln() / KILO.ln()).floor() as usize).min(UNITS.len());
    format!(
        "{} {}",
        compact(bytes as f64 / KILO.powi(step as i32)),
        UNITS[step - 1]
    )
}

pub fn file_extension(name: &str) -> String {
    match name.rfind('.') {
        Some(dot) if dot > 0 && dot + 1 < name.len() => name[dot + 1..].to_lowercase(),
        _ => String::new(),
    }
}

pub fn extension_tag(name: &str) -> String {
    let tag: String = file_extension(name).chars().take(TAG_LETTERS).collect();
    if tag.is_empty() {
        "file".to_string()
    } else {
        tag
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sizes_read_like_the_web_ones() {
        for (bytes, shown) in [
            (0, "0 B"),
            (512, "512 B"),
            (4096, "4 KB"),
            (4300, "4.2 KB"),
            (49152, "48 KB"),
            (186368, "182 KB"),
            (1153434, "1.1 MB"),
            (20 * 1024 * 1024, "20 MB"),
        ] {
            assert_eq!(format_size(bytes), shown, "{bytes} bytes");
        }
    }

    #[test]
    fn extensions_are_lowercase_and_empty_for_dotfiles_and_trailing_dots() {
        for (name, ext) in [
            ("localhost.har", "har"),
            ("Schema.SQL", "sql"),
            ("archive.tar.gz", "gz"),
            ("Makefile", ""),
            (".env", ""),
            ("trailing.", ""),
        ] {
            assert_eq!(file_extension(name), ext, "{name}");
        }
    }

    #[test]
    fn the_tag_has_at_most_four_letters_and_says_file_without_an_extension() {
        assert_eq!(extension_tag("auth-flow.pdf"), "pdf");
        assert_eq!(extension_tag("notes.markdown"), "mark");
        assert_eq!(extension_tag("Makefile"), "file");
    }
}
