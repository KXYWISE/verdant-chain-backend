use serde::{Deserialize, Serialize};

/// A decoded Soroban contract event as delivered by the chain event source.
/// Decoded topics/data are JSON to keep the raw store agnostic to payload shape.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ChainEvent {
    pub contract_id: String,
    pub contract_name: String,
    pub event_name: String,
    pub ledger_sequence: i64,
    pub operation_index: i32,
    pub event_index: i32,
    pub topics: serde_json::Value,
    pub data: serde_json::Value,
}

/// Per-contract monotonic ingestion position. Events are ordered by
/// (ledger_sequence, operation_index, event_index); the cursor points at the
/// last processed event so ingestion resumes strictly after it.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct Cursor {
    pub ledger_sequence: i64,
    pub operation_index: i32,
    pub event_index: i32,
}

impl Cursor {
    pub const START: Cursor = Cursor {
        ledger_sequence: 0,
        operation_index: -1,
        event_index: -1,
    };

    pub fn is_after(&self, other: &Cursor) -> bool {
        self.ledger_sequence > other.ledger_sequence
            || (self.ledger_sequence == other.ledger_sequence
                && (self.operation_index > other.operation_index
                    || (self.operation_index == other.operation_index
                        && self.event_index > other.event_index)))
    }
}

#[cfg(test)]
mod tests {
    use super::Cursor;

    #[test]
    fn cursor_ordering() {
        let a = Cursor {
            ledger_sequence: 100,
            operation_index: 0,
            event_index: 0,
        };
        let b = Cursor {
            ledger_sequence: 100,
            operation_index: 0,
            event_index: 1,
        };
        let c = Cursor {
            ledger_sequence: 101,
            operation_index: 0,
            event_index: 0,
        };
        assert!(!a.is_after(&a));
        assert!(b.is_after(&a));
        assert!(c.is_after(&b));
        assert!(!a.is_after(&c));
    }
}
