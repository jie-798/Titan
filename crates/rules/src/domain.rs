/// 域名匹配工具

/// 标准化域名
pub fn normalize_domain(domain: &str) -> String {
    domain.trim().trim_start_matches("www.").to_lowercase()
}

/// 匹配域名后缀
pub fn match_suffix(domain: &str, suffix: &str) -> bool {
    let domain = normalize_domain(domain);
    let suffix = normalize_domain(suffix);

    if domain == suffix {
        return true;
    }

    domain.ends_with(&format!(".{}", suffix))
}

/// 匹配域名关键字
pub fn match_keyword(domain: &str, keyword: &str) -> bool {
    domain.to_lowercase().contains(&keyword.to_lowercase())
}
