use super::*;

#[test]
fn normalizing_lowercases_and_strips_numbers_ids_and_paths() {
    assert_eq!(
        normalize_phrase("Fix the  login bug in src/auth/login.ts, issue 1234!"),
        "fix the login bug in issue"
    );
    assert_eq!(
        normalize_phrase("rerun task fix-2 at 3f2a9c10-1111-2222-3333-444455556666"),
        "rerun task at"
    );
    assert_eq!(normalize_phrase("  /tmp/x 42  "), "");
}

#[test]
fn normalizing_masks_secrets_before_anything_else() {
    let n = normalize_phrase("use token=hunter2hunter2 to push");
    assert!(!n.contains("hunter"), "{n}");
    assert!(n.contains("push"));
}
