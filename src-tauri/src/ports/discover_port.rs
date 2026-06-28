use crate::domain::discover::{DiscoverableItem, DiscoverableKind};
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

#[cfg_attr(test, mockall::automock)]
#[async_trait]
pub trait DiscoverService: Send + Sync {
    async fn warm(&self) -> Result<(), String>;

    async fn invalidate(&self);

    async fn search(
        &self,
        query: &str,
        kinds: Option<Vec<DiscoverableKind>>,
        limit: usize,
    ) -> Vec<DiscoverableItem>;
}

#[derive(Default, Clone)]
pub struct AliasOverrides {
    inner: Arc<RwLock<HashMap<String, Vec<String>>>>,
}

impl AliasOverrides {
    pub fn replace(&self, aliases: HashMap<String, Vec<String>>) {
        if let Ok(mut guard) = self.inner.write() {
            *guard = aliases;
        }
    }

    pub fn snapshot(&self) -> HashMap<String, Vec<String>> {
        self.inner.read().map(|g| g.clone()).unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::discover::{DiscoverableKind, DiscoverableLaunch};

    #[tokio::test]
    async fn automock_records_search_calls() {
        let mut mock = MockDiscoverService::new();
        mock.expect_search()
            .times(1)
            .withf(|q, kinds, l| q == "hello" && kinds.is_none() && *l == 5)
            .returning(|_, _, _| {
                vec![DiscoverableItem {
                    kind: DiscoverableKind::App,
                    id: "x".to_string(),
                    name: "x".to_string(),
                    description: None,
                    icon: None,
                    keywords: Vec::new(),
                    score: 1.0,
                    source: "test".to_string(),
                    launch: DiscoverableLaunch::App {
                        exec: "x".to_string(),
                    },
                    category: None,
                    aliases: Vec::new(),
                }]
            });

        let items = mock.search("hello", None, 5).await;
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].kind, DiscoverableKind::App);
    }

    #[tokio::test]
    async fn automock_records_search_calls_with_kind_filter() {
        let mut mock = MockDiscoverService::new();
        mock.expect_search()
            .times(1)
            .withf(|q, kinds, l| {
                q == "x"
                    && kinds.as_ref().map(|v| v.as_slice())
                        == Some(&[DiscoverableKind::App][..])
                    && *l == 3
            })
            .returning(|_, _, _| Vec::new());
        let items = mock
            .search("x", Some(vec![DiscoverableKind::App]), 3)
            .await;
        assert!(items.is_empty());
    }

    #[test]
    fn alias_overrides_round_trip() {
        let overrides = AliasOverrides::default();
        assert!(overrides.snapshot().is_empty());
        overrides.replace(HashMap::from([(
            "google-chrome".to_string(),
            vec!["browser".to_string()],
        )]));
        let snap = overrides.snapshot();
        assert_eq!(snap.get("google-chrome").map(|v| v.as_slice()), Some(&["browser".to_string()][..]));
    }
}