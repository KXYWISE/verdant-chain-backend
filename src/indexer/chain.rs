use std::collections::BTreeMap;
use std::sync::Mutex;

use async_trait::async_trait;

use crate::indexer::model::{ChainEvent, Cursor};

/// Error from a chain event source (Soroban RPC or stub).
#[derive(Debug, thiserror::Error)]
pub enum ChainEventsError {
    #[error("event source not available: {0}")]
    Unavailable(String),
    #[error("internal: {0}")]
    Internal(String),
}

/// Abstraction over a contract-event stream (Plan §indexer architecture).
/// Mirrors `IdentityChain`: a stub implements the trait for tests; the real
/// Soroban RPC subscriber is wired in the same step as the real chain.
#[async_trait]
pub trait ChainEvents: Send + Sync {
    /// Returns decoded events for `contract_id` ordered by
    /// (ledger_sequence, operation_index, event_index), strictly after `cursor`.
    async fn events_after(
        &self,
        contract_id: &str,
        cursor: Cursor,
    ) -> Result<Vec<ChainEvent>, ChainEventsError>;

    /// Current ledger head, used for the trusted finality cutoff.
    async fn head_ledger(&self) -> Result<i64, ChainEventsError>;
}

/// In-memory event source for unit/integration tests (mirrors `StubChain`).
#[derive(Debug, Default)]
pub struct StubEventSource {
    inner: Mutex<StubInner>,
}

#[derive(Debug, Default)]
struct StubInner {
    events: BTreeMap<(i64, i32, i32), ChainEvent>,
    head: i64,
}

impl StubEventSource {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&self, event: ChainEvent) {
        let mut inner = self.inner.lock().unwrap();
        let key = (
            event.ledger_sequence,
            event.operation_index,
            event.event_index,
        );
        inner.events.insert(key, event);
    }

    pub fn set_head(&self, ledger: i64) {
        self.inner.lock().unwrap().head = ledger;
    }
}

#[async_trait]
impl ChainEvents for StubEventSource {
    async fn events_after(
        &self,
        contract_id: &str,
        cursor: Cursor,
    ) -> Result<Vec<ChainEvent>, ChainEventsError> {
        let inner = self.inner.lock().unwrap();
        Ok(inner
            .events
            .iter()
            .map(|((_l, _o, _e), ev)| ev.clone())
            .filter(|ev| ev.contract_id == contract_id)
            .filter(|ev| {
                Cursor {
                    ledger_sequence: ev.ledger_sequence,
                    operation_index: ev.operation_index,
                    event_index: ev.event_index,
                }
                .is_after(&cursor)
            })
            .collect())
    }

    async fn head_ledger(&self) -> Result<i64, ChainEventsError> {
        Ok(self.inner.lock().unwrap().head)
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::StubEventSource;
    use crate::indexer::chain::ChainEvents;
    use crate::indexer::model::{ChainEvent, Cursor};

    fn ev(ledger: i64, op: i32, ev: i32, name: &str) -> ChainEvent {
        ChainEvent {
            contract_id: "C1".into(),
            contract_name: "verification".into(),
            event_name: name.into(),
            ledger_sequence: ledger,
            operation_index: op,
            event_index: ev,
            topics: json!([1]),
            data: json!([]),
        }
    }

    #[tokio::test]
    async fn returns_events_after_cursor() {
        let source = StubEventSource::new();
        source.push(ev(100, 0, 0, "VerificationCreated"));
        source.push(ev(100, 0, 1, "VerificationCreated"));
        source.push(ev(101, 0, 0, "VerificationCreated"));

        let all = source.events_after("C1", Cursor::START).await.unwrap();
        assert_eq!(all.len(), 3);

        let after_first = source
            .events_after(
                "C1",
                Cursor {
                    ledger_sequence: 100,
                    operation_index: 0,
                    event_index: 0,
                },
            )
            .await
            .unwrap();
        assert_eq!(after_first.len(), 2);

        let head = source.head_ledger().await.unwrap();
        assert_eq!(head, 0);
        source.set_head(200);
        assert_eq!(source.head_ledger().await.unwrap(), 200);
    }

    #[tokio::test]
    async fn filters_by_contract() {
        let source = StubEventSource::new();
        let mut other = ev(100, 0, 0, "VerificationCreated");
        other.contract_id = "C2".into();
        source.push(ev(100, 0, 1, "VerificationCreated"));
        source.push(other);

        let events = source.events_after("C1", Cursor::START).await.unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].contract_id, "C1");
    }
}
