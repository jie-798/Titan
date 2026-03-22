use crate::domain::{match_keyword, match_suffix};

#[test]
fn test_suffix_match() {
    assert!(match_suffix("www.google.com", "google.com"));
    assert!(match_suffix("mail.google.com", "google.com"));
    assert!(match_suffix("google.com", "google.com"));
    assert!(!match_suffix("facebook.com", "google.com"));
}

#[test]
fn test_keyword_match() {
    assert!(match_keyword("www.google.com", "google"));
    assert!(!match_keyword("www.facebook.com", "google"));
}
