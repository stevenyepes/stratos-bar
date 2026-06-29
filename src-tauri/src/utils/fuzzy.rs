use crate::domain::apps::{AppEntry, MatchedField, ScoredApp};

const NAME_PREFIX_BASE: f32 = 1.00;
const NAME_CONTAINS_BASE: f32 = 0.85;
const KEYWORD_BASE: f32 = 0.70;
const GENERIC_NAME_BASE: f32 = 0.55;
const DESCRIPTION_BASE: f32 = 0.40;
const EXEC_BASE: f32 = 0.25;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FieldStrength {
    Prefix,
    Contains,
    Subsequence,
}

pub fn is_subsequence(query: &str, target: &str) -> bool {
    if query.is_empty() {
        return true;
    }
    let mut q = query.chars();
    let mut needle = q.next();
    for c in target.chars() {
        if Some(c) == needle {
            needle = q.next();
            if needle.is_none() {
                return true;
            }
        }
    }
    needle.is_none()
}

fn classify(query: &str, target: &str) -> Option<FieldStrength> {
    let q = query.to_lowercase();
    let t = target.to_lowercase();
    if q.is_empty() || t.is_empty() {
        return None;
    }
    if t.starts_with(&q) {
        return Some(FieldStrength::Prefix);
    }
    if t.contains(&q) {
        return Some(FieldStrength::Contains);
    }
    if is_subsequence(&q, &t) {
        return Some(FieldStrength::Subsequence);
    }
    None
}

fn strength_score(strength: FieldStrength, query: &str, target: &str) -> f32 {
    let qlen = query.chars().count() as f32;
    let tlen = target.chars().count().max(1) as f32;
    let compactness = qlen / tlen;
    match strength {
        FieldStrength::Prefix => 1.0 + compactness,
        FieldStrength::Contains => {
            let q_lower = query.to_lowercase();
            let t_lower = target.to_lowercase();
            let pos = t_lower.find(&q_lower).unwrap_or(0) as f32;
            let early_bonus = 1.0 - (pos / tlen);
            0.85 + compactness * 0.5 + early_bonus * 0.2
        }
        FieldStrength::Subsequence => 0.5 + compactness * 0.3,
    }
}

fn base_score_for(field: &MatchedField) -> f32 {
    match field {
        MatchedField::NamePrefix => NAME_PREFIX_BASE,
        MatchedField::NameContains => NAME_CONTAINS_BASE,
        MatchedField::Keyword => KEYWORD_BASE,
        MatchedField::GenericName => GENERIC_NAME_BASE,
        MatchedField::Description => DESCRIPTION_BASE,
        MatchedField::Exec => EXEC_BASE,
    }
}

pub fn score_app(app: &AppEntry, query: &str) -> Option<(f32, MatchedField)> {
    let trimmed = query.trim();
    if trimmed.is_empty() {
        return None;
    }

    if matches!(classify(trimmed, &app.name), Some(FieldStrength::Prefix)) {
        let score = base_score_for(&MatchedField::NamePrefix)
            + strength_score(FieldStrength::Prefix, trimmed, &app.name) - 1.0;
        return Some((score, MatchedField::NamePrefix));
    }

    if let Some(strength) = classify(trimmed, &app.name) {
        let score = base_score_for(&MatchedField::NameContains)
            + strength_score(strength, trimmed, &app.name) - NAME_CONTAINS_BASE;
        return Some((score, MatchedField::NameContains));
    }

    for kw in &app.keywords {
        if let Some(strength) = classify(trimmed, kw) {
            let score = base_score_for(&MatchedField::Keyword)
                + strength_score(strength, trimmed, kw) - KEYWORD_BASE;
            return Some((score, MatchedField::Keyword));
        }
    }

    if let Some(gn) = app.generic_name.as_deref() {
        if let Some(strength) = classify(trimmed, gn) {
            let score = base_score_for(&MatchedField::GenericName)
                + strength_score(strength, trimmed, gn) - GENERIC_NAME_BASE;
            return Some((score, MatchedField::GenericName));
        }
    }

    if let Some(desc) = app.description.as_deref() {
        if let Some(strength) = classify(trimmed, desc) {
            let score = base_score_for(&MatchedField::Description)
                + strength_score(strength, trimmed, desc) - DESCRIPTION_BASE;
            return Some((score, MatchedField::Description));
        }
    }

    if let Some(strength) = classify(trimmed, &app.exec) {
        let score = base_score_for(&MatchedField::Exec)
            + strength_score(strength, trimmed, &app.exec) - EXEC_BASE;
        return Some((score, MatchedField::Exec));
    }

    None
}

pub fn rank_apps(apps: &[AppEntry], query: &str, limit: usize) -> Vec<ScoredApp> {
    if query.trim().is_empty() {
        return Vec::new();
    }

    let mut scored: Vec<ScoredApp> = apps
        .iter()
        .filter_map(|app| {
            let (score, matched_field) = score_app(app, query)?;
            Some(ScoredApp {
                app: app.clone(),
                score,
                matched_field,
            })
        })
        .collect();

    scored.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.app.name.to_lowercase().cmp(&b.app.name.to_lowercase()))
    });
    scored.truncate(limit);
    scored
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::apps::AppSource;

    fn make_app(name: &str) -> AppEntry {
        AppEntry {
            id: name.to_lowercase(),
            name: name.to_string(),
            generic_name: None,
            description: None,
            keywords: Vec::new(),
            exec: name.to_lowercase(),
            try_exec: None,
            icon: None,
            categories: Vec::new(),
            startup_wm_class: None,
            source: AppSource::Desktop,
            path: format!("/usr/share/applications/{name}.desktop"),
            ..Default::default()
        }
    }

    #[test]
    fn subsequence_empty_query_is_trivially_true() {
        assert!(is_subsequence("", "anything"));
    }

    #[test]
    fn subsequence_matches_strict_order() {
        assert!(is_subsequence("chr", "chrome"));
        assert!(is_subsequence("cde", "abcdef"));
        assert!(!is_subsequence("xyz", "chrome"));
        assert!(!is_subsequence("crx", "chrome"));
    }

    #[test]
    fn classify_prefers_prefix_then_contains_then_subsequence() {
        assert_eq!(classify("chr", "chrome"), Some(FieldStrength::Prefix));
        assert_eq!(classify("rom", "chrome"), Some(FieldStrength::Contains));
        assert_eq!(classify("cme", "chrome"), Some(FieldStrength::Subsequence));
        assert_eq!(classify("zzz", "chrome"), None);
    }

    #[test]
    fn classify_is_case_insensitive() {
        assert_eq!(classify("CHR", "Chrome"), Some(FieldStrength::Prefix));
        assert_eq!(classify("ChR", "chrome"), Some(FieldStrength::Prefix));
    }

    #[test]
    fn classify_returns_none_for_empty_inputs() {
        assert_eq!(classify("", "chrome"), None);
        assert_eq!(classify("chr", ""), None);
    }

    #[test]
    fn score_app_returns_none_for_empty_query() {
        let app = make_app("Chrome");
        assert!(score_app(&app, "").is_none());
        assert!(score_app(&app, "   ").is_none());
    }

    #[test]
    fn score_app_returns_none_for_no_match() {
        let app = make_app("Chrome");
        assert!(score_app(&app, "xyzpdq").is_none());
    }

    #[test]
    fn name_prefix_outranks_name_contains() {
        let prefix = make_app("Code");
        let contains = make_app("VisualStudioCode");
        let p = score_app(&prefix, "code").unwrap();
        let c = score_app(&contains, "code").unwrap();
        assert_eq!(p.1, MatchedField::NamePrefix);
        assert_eq!(c.1, MatchedField::NameContains);
        assert!(p.0 > c.0, "prefix {p:?} should beat contains {c:?}");
    }

    #[test]
    fn rank_orders_chrome_before_chromium_on_chr() {
        let apps = vec![
            make_app("Chromium"),
            make_app("Chrome"),
            make_app("Cursor"),
            make_app("Figma"),
            make_app("Code"),
        ];
        let ranked = rank_apps(&apps, "chr", 10);
        assert_eq!(ranked.len(), 2);
        assert_eq!(ranked[0].app.name, "Chrome");
        assert_eq!(ranked[1].app.name, "Chromium");
        assert_eq!(ranked[0].matched_field, MatchedField::NamePrefix);
        assert_eq!(ranked[1].matched_field, MatchedField::NamePrefix);
        assert!(ranked[0].score > ranked[1].score);
    }

    #[test]
    fn rank_excludes_non_matches() {
        let apps = vec![make_app("Figma"), make_app("Cursor"), make_app("Chrome")];
        let ranked = rank_apps(&apps, "chr", 10);
        let names: Vec<&str> = ranked.iter().map(|r| r.app.name.as_str()).collect();
        assert_eq!(names, vec!["Chrome"]);
    }

    #[test]
    fn rank_caps_at_limit() {
        let apps: Vec<AppEntry> = (0..50)
            .map(|i| make_app(&format!("App{i}")))
            .collect();
        let ranked = rank_apps(&apps, "app", 5);
        assert_eq!(ranked.len(), 5);
    }

    #[test]
    fn rank_respects_field_priority() {
        let mut keyword_app = make_app("Foo");
        keyword_app.keywords = vec!["chrome".to_string()];
        let name_prefix = make_app("Chrome");
        let mut exec = make_app("Bar");
        exec.exec = "chrome-runner".to_string();

        let apps = vec![keyword_app, name_prefix, exec];
        let ranked = rank_apps(&apps, "chr", 10);
        assert_eq!(ranked.len(), 3);
        assert_eq!(ranked[0].matched_field, MatchedField::NamePrefix);
        assert_eq!(ranked[1].matched_field, MatchedField::Keyword);
        assert_eq!(ranked[2].matched_field, MatchedField::Exec);
    }

    #[test]
    fn description_match_returns_description_field() {
        let mut app = make_app("Foo");
        app.description = Some("A friendly Chromium-based browser".to_string());
        let (_, field) = score_app(&app, "chrom").unwrap();
        assert_eq!(field, MatchedField::Description);
    }

    #[test]
    fn generic_name_match_returns_generic_name_field() {
        let mut app = make_app("Foo");
        app.generic_name = Some("Web Browser".to_string());
        let (_, field) = score_app(&app, "browser").unwrap();
        assert_eq!(field, MatchedField::GenericName);
    }

    #[test]
    fn keyword_match_returns_keyword_field() {
        let mut app = make_app("Foo");
        app.keywords = vec!["browser".to_string()];
        let (_, field) = score_app(&app, "browser").unwrap();
        assert_eq!(field, MatchedField::Keyword);
    }

    #[test]
    fn subsequence_only_in_name_classifies_as_name_contains() {
        let app = make_app("Chromatic");
        let (_, field) = score_app(&app, "ctc").unwrap();
        assert_eq!(field, MatchedField::NameContains);
    }

    #[test]
    fn exec_only_match_uses_exec_field() {
        let mut app = make_app("Foo");
        app.exec = "google-chrome-stable".to_string();
        let (_, field) = score_app(&app, "stable").unwrap();
        assert_eq!(field, MatchedField::Exec);
    }

    #[test]
    fn rank_apps_empty_input_returns_empty() {
        let ranked = rank_apps(&[], "chrome", 10);
        assert!(ranked.is_empty());
    }

    #[test]
    fn rank_apps_whitespace_query_returns_empty() {
        let apps = vec![make_app("Chrome")];
        let ranked = rank_apps(&apps, "   ", 10);
        assert!(ranked.is_empty());
    }

    #[test]
    fn ties_break_alphabetically() {
        let apps = vec![make_app("Chrome B"), make_app("Chrome A")];
        let ranked = rank_apps(&apps, "chr", 10);
        assert_eq!(ranked[0].app.name, "Chrome A");
        assert_eq!(ranked[1].app.name, "Chrome B");
    }

    #[test]
    fn handles_5000_apps_within_budget() {
        let apps: Vec<AppEntry> = (0..5000)
            .map(|i| make_app(&format!("App{i:04}")))
            .collect();
        let start = std::time::Instant::now();
        let ranked = rank_apps(&apps, "1234", 50);
        let elapsed = start.elapsed();
        assert!(elapsed.as_millis() < 200, "ranking took too long: {elapsed:?}");
        assert!(!ranked.is_empty());
        assert!(ranked.len() <= 50);
    }
}