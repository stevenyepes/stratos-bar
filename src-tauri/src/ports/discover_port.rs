use crate::domain::action::Action;
use crate::domain::config::{AiTool, ScriptConfig};
use crate::domain::discover::{BrowseSections, ScoredDiscoverable};
use std::collections::HashMap;

#[cfg_attr(test, mockall::automock)]
pub trait DiscoverService: Send + Sync {
    fn search(&self, query: &str, limit: usize) -> Vec<ScoredDiscoverable>;

    fn browse(
        &self,
        scripts: Vec<ScriptConfig>,
        ai_tools: Vec<AiTool>,
        recent_files: Vec<Action>,
        shortcuts: HashMap<String, String>,
    ) -> BrowseSections;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::discover::{DiscoverableItem, DiscoverableKind, MatchedField};

    #[test]
    fn mock_discover_service_can_be_constructed() {
        let mut mock = MockDiscoverService::new();
        mock.expect_search()
            .times(1)
            .returning(|_, _| Vec::<ScoredDiscoverable>::new());
        let result = mock.search("anything", 10);
        assert!(result.is_empty());
    }

    #[test]
    fn mock_discover_service_browse_returns_structured_sections() {
        let mut mock = MockDiscoverService::new();
        let expected = BrowseSections::default();
        mock.expect_browse().times(1).returning(move |_, _, _, _| {
            BrowseSections {
                apps_by_category: HashMap::from([(
                    "Development".to_string(),
                    vec![DiscoverableItem {
                        id: "code".to_string(),
                        name: "Code".to_string(),
                        kind: DiscoverableKind::App,
                        category: Some("Development".to_string()),
                        aliases: vec!["editor".to_string()],
                        ..Default::default()
                    }],
                )]),
                ..expected.clone()
            }
        });
        let result: BrowseSections = mock.browse(vec![], vec![], vec![], HashMap::new());
        assert!(result.apps_by_category.contains_key("Development"));
    }

    #[test]
    fn mock_discover_service_search_with_scored_items() {
        let mut mock = MockDiscoverService::new();
        mock.expect_search().returning(|q, limit| {
            if q == "chrome" {
                vec![ScoredDiscoverable {
                    item: DiscoverableItem {
                        id: "google-chrome".to_string(),
                        name: "Chrome".to_string(),
                        kind: DiscoverableKind::App,
                        ..Default::default()
                    },
                    score: 1.5,
                    matched_field: MatchedField::AliasExact,
                }]
            } else {
                Vec::new()
            }
            .into_iter()
            .take(limit)
            .collect()
        });
        let hits = mock.search("chrome", 10);
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].item.id, "google-chrome");
        let none = mock.search("nope", 10);
        assert!(none.is_empty());
    }
}