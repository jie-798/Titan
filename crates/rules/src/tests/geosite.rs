use std::path::PathBuf;
use std::thread;
use std::time::Duration;

use crate::geosite::GeositeMatcher;

#[test]
fn parses_simple_category_file() {
    let dir = temp_geosite_dir();
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("google.txt");
    std::fs::write(
        &path,
        "\
domain:accounts.google.com
suffix:google.com
keyword:youtube
plain-example.com
",
    )
    .unwrap();

    let matcher = GeositeMatcher::new(&dir);
    assert!(matcher.matches("google", "accounts.google.com"));
    assert!(matcher.matches("google", "mail.google.com"));
    assert!(matcher.matches("google", "m.youtube.com"));
    assert!(matcher.matches("google", "plain-example.com"));
    assert!(!matcher.matches("google", "facebook.com"));

    let _ = std::fs::remove_file(path);
    let _ = std::fs::remove_dir(dir);
}

#[test]
fn reloads_when_file_changes() {
    let dir = temp_geosite_dir();
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("google.txt");
    std::fs::write(&path, "suffix:google.com\n").unwrap();

    let matcher = GeositeMatcher::new(&dir);
    assert!(matcher.matches("google", "mail.google.com"));
    assert!(!matcher.matches("google", "mail.youtube.com"));

    thread::sleep(Duration::from_secs(5) + Duration::from_millis(50));
    std::fs::write(&path, "suffix:youtube.com\n").unwrap();
    thread::sleep(Duration::from_millis(50));

    assert!(!matcher.matches("google", "mail.google.com"));
    assert!(matcher.matches("google", "mail.youtube.com"));

    let _ = std::fs::remove_file(path);
    let _ = std::fs::remove_dir(dir);
}

fn temp_geosite_dir() -> PathBuf {
    let mut path = std::env::temp_dir();
    let unique = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    path.push(format!(
        "titan-geosite-test-{}-{}",
        std::process::id(),
        unique
    ));
    path
}
