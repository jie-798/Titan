use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::RwLock;
use std::time::{Duration, Instant, SystemTime};

use crate::domain::{match_keyword, match_suffix, normalize_domain};

pub const DEFAULT_GEOSITE_DIR: &str = "data/geosite";
const GEOSITE_REFRESH_INTERVAL: Duration = Duration::from_secs(5);

#[derive(Debug, Clone, Default)]
struct GeositeCategory {
    domains: Vec<String>,
    suffixes: Vec<String>,
    keywords: Vec<String>,
}

impl GeositeCategory {
    fn matches(&self, host: &str) -> bool {
        let normalized = normalize_domain(host);

        self.domains.iter().any(|domain| normalized == *domain)
            || self
                .suffixes
                .iter()
                .any(|suffix| match_suffix(&normalized, suffix))
            || self
                .keywords
                .iter()
                .any(|keyword| match_keyword(&normalized, keyword))
    }
}

#[derive(Debug)]
struct CategoryState {
    category: Option<GeositeCategory>,
    modified: Option<SystemTime>,
    last_checked: Option<Instant>,
}

#[derive(Debug)]
pub struct GeositeMatcher {
    base_dir: PathBuf,
    categories: RwLock<HashMap<String, CategoryState>>,
}

impl GeositeMatcher {
    pub fn new<P: AsRef<Path>>(base_dir: P) -> Self {
        Self {
            base_dir: base_dir.as_ref().to_path_buf(),
            categories: RwLock::new(HashMap::new()),
        }
    }

    pub fn matches(&self, category: &str, host: &str) -> bool {
        self.refresh_category_if_needed(category);

        self.categories
            .read()
            .ok()
            .and_then(|categories| {
                categories
                    .get(&normalize_category(category))
                    .and_then(|state| state.category.as_ref())
                    .map(|category| category.matches(host))
            })
            .unwrap_or(false)
    }

    fn refresh_category_if_needed(&self, category: &str) {
        let key = normalize_category(category);
        let path = self.category_path(&key);

        let should_check = self
            .categories
            .read()
            .ok()
            .and_then(|categories| categories.get(&key).map(|state| state.last_checked))
            .flatten()
            .is_none_or(|last_checked| last_checked.elapsed() >= GEOSITE_REFRESH_INTERVAL);

        if !should_check {
            return;
        }

        let modified = match file_modified(&path) {
            Ok(modified) => modified,
            Err(err) => {
                tracing::warn!("failed to stat GEOSITE file {}: {}", path.display(), err);
                self.update_check_time(&key);
                return;
            }
        };

        let reload_needed = self
            .categories
            .read()
            .ok()
            .and_then(|categories| categories.get(&key).map(|state| state.modified != modified))
            .unwrap_or(true);

        self.update_check_time(&key);

        if !reload_needed {
            return;
        }

        match load_category_from_path(&path) {
            Ok(category_data) => {
                tracing::info!("reloaded GEOSITE category {} from {}", key, path.display());
                if let Ok(mut categories) = self.categories.write() {
                    categories.insert(
                        key,
                        CategoryState {
                            category: Some(category_data),
                            modified,
                            last_checked: Some(Instant::now()),
                        },
                    );
                }
            }
            Err(err) => {
                tracing::warn!(
                    "failed to reload GEOSITE category {} from {}: {}",
                    key,
                    path.display(),
                    err
                );
            }
        }
    }

    fn update_check_time(&self, category: &str) {
        if let Ok(mut categories) = self.categories.write() {
            categories
                .entry(category.to_string())
                .and_modify(|state| state.last_checked = Some(Instant::now()))
                .or_insert(CategoryState {
                    category: None,
                    modified: None,
                    last_checked: Some(Instant::now()),
                });
        }
    }

    fn category_path(&self, category: &str) -> PathBuf {
        self.base_dir.join(format!("{}.txt", category))
    }
}

fn load_category_from_path(path: &Path) -> anyhow::Result<GeositeCategory> {
    let content = std::fs::read_to_string(path)?;
    load_category_from_str(&content)
}

fn load_category_from_str(content: &str) -> anyhow::Result<GeositeCategory> {
    let mut category = GeositeCategory::default();

    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        let (kind, value) = match line.split_once(':') {
            Some((kind, value)) => (kind.trim(), value.trim()),
            None => ("suffix", line),
        };

        let value = normalize_domain(value);
        if value.is_empty() {
            continue;
        }

        match kind {
            "full" | "domain" => category.domains.push(value),
            "keyword" => category.keywords.push(value),
            "suffix" | "domain-suffix" => category.suffixes.push(value),
            _ => category.suffixes.push(value),
        }
    }

    Ok(category)
}

fn normalize_category(category: &str) -> String {
    category.trim().to_ascii_lowercase()
}

fn file_modified(path: &Path) -> anyhow::Result<Option<SystemTime>> {
    Ok(std::fs::metadata(path)
        .ok()
        .and_then(|meta| meta.modified().ok()))
}
