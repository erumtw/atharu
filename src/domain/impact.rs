//! The impact line: one sentence for a perf review, shared by the epic page and the report.

/// `Did {title} → {metric} {before}→{after} → {urls}`; `None` until metric, before and after are all filled.
pub fn impact_line(
    title: &str,
    metric: &str,
    before: &str,
    after: &str,
    urls: &[String],
) -> Option<String> {
    if [metric, before, after].iter().any(|s| s.trim().is_empty()) {
        return None;
    }
    let mut s = format!("Did {title} → {metric} {before}→{after}");
    if !urls.is_empty() {
        s.push_str(" → ");
        s.push_str(&urls.join(", "));
    }
    Some(s)
}

#[cfg(test)]
mod tests {
    use super::impact_line;

    #[test]
    fn builds_line() {
        let urls = vec![
            "https://gh.io/pr/412".to_string(),
            "https://graf.io/p95".to_string(),
        ];
        assert_eq!(
            impact_line("Checkout latency", "checkout p95", "1.8s", "0.6s", &urls).as_deref(),
            Some(
                "Did Checkout latency → checkout p95 1.8s→0.6s → https://gh.io/pr/412, https://graf.io/p95"
            )
        );
        assert_eq!(
            impact_line("X", "m", "1", "2", &[]).as_deref(),
            Some("Did X → m 1→2")
        );
        assert_eq!(impact_line("X", "m", "", "2", &urls), None);
    }
}
