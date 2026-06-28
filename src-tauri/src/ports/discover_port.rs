use crate::domain::discover::DiscoverableItem;
use async_trait::async_trait;

#[cfg_attr(test, mockall::automock)]
#[async_trait]
pub trait DiscoverService: Send + Sync {
    async fn warm(&self) -> Result<(), String>;

    async fn invalidate(&self);

    async fn search(&self, query: &str, limit: usize) -> Vec<DiscoverableItem>;
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
            .withf(|q, l| q == "hello" && *l == 5)
            .returning(|_, _| {
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
                }]
            });

        let items = mock.search("hello", 5).await;
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].kind, DiscoverableKind::App);
    }
}