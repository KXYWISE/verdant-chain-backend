use sqlx::PgPool;
use std::sync::Arc;
use std::time::Duration;
use tracing::info;

use crate::indexer::chain::{ChainEvents, ChainEventsError};
use crate::indexer::model::Cursor;
use crate::indexer::projections;
use crate::indexer::store;

/// Trusted finality cutoff (ledgers): projection writes finalize only for
/// ledger_sequence older than head - cutoff (Agent #4 decision, Q1).
pub const TRUSTED_CUTOFF: i64 = 10;

/// Poll interval for the background indexer loop.
pub const POLL_INTERVAL: Duration = Duration::from_secs(5);

/// Spawns the background ingestion loop: for each configured contract, pulls
/// new events and applies finality-gated projections on every tick. Logs
/// transient errors and keeps polling.
pub fn spawn_indexer(pool: PgPool, source: Arc<dyn ChainEvents>, contract_ids: Vec<String>) {
    tokio::spawn(async move {
        loop {
            for contract_id in &contract_ids {
                if let Err(err) = ingest(&pool, source.as_ref(), contract_id).await {
                    tracing::warn!(contract_id, %err, "indexer ingest error");
                }
            }
            tokio::time::sleep(POLL_INTERVAL).await;
        }
    });
}

/// Ingests new events for a contract: pulls from the event source, appends
/// to the raw store, advances the cursor, and applies finality-gated
/// projections. Idempotent for replays.
///
/// The cursor always advances to the last event pulled from the source, even
/// if that event is inside the un-finalized window. Projections are applied in
/// a separate idempotent pass over the stored events at or below the finality
/// floor, so events that were pulled while un-finalized are applied once they
/// cross the floor on a later ingest.
pub async fn ingest(
    pool: &PgPool,
    source: &dyn ChainEvents,
    contract_id: &str,
) -> Result<usize, ChainEventsError> {
    let cursor = store::get_cursor(pool, contract_id)
        .await
        .map_err(|e| ChainEventsError::Internal(e.to_string()))?;
    let events = source.events_after(contract_id, cursor).await?;
    if events.is_empty() {
        return Ok(0);
    }

    let head = source.head_ledger().await?;
    let finality_floor = head - TRUSTED_CUTOFF;

    store::append_events(pool, &events)
        .await
        .map_err(|e| ChainEventsError::Internal(e.to_string()))?;

    // Advance the cursor to the last event pulled, regardless of finality.
    let last = events
        .last()
        .map(|e| Cursor {
            ledger_sequence: e.ledger_sequence,
            operation_index: e.operation_index,
            event_index: e.event_index,
        })
        .unwrap_or(cursor);

    store::set_cursor(pool, contract_id, last)
        .await
        .map_err(|e| ChainEventsError::Internal(e.to_string()))?;

    // Projections: apply every stored event at or below the finality floor.
    // Idempotent upserts make re-application safe.
    let finalized = store::finalized_events(pool, contract_id, finality_floor)
        .await
        .map_err(|e| ChainEventsError::Internal(e.to_string()))?;
    let mut applied = 0usize;
    for event in &finalized {
        if projections::apply_event(pool, event)
            .await
            .map_err(|e| ChainEventsError::Internal(e.to_string()))?
        {
            applied += 1;
        }
    }

    info!(
        contract_id,
        count = events.len(),
        applied,
        "indexer ingested batch"
    );

    Ok(events.len())
}
