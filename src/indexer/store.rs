use sqlx::PgPool;

use crate::indexer::model::{ChainEvent, Cursor};

/// Appends raw events idempotently. The natural key
/// (contract_id, ledger_sequence, operation_index, event_index) makes replays
/// and re-org re-ingestion safe (INSERT ... ON CONFLICT DO NOTHING).
pub async fn append_events(pool: &PgPool, events: &[ChainEvent]) -> Result<(), sqlx::Error> {
    for event in events {
        sqlx::query!(
            r#"
            INSERT INTO indexer.indexed_events
                (contract_id, contract_name, event_name, ledger_sequence,
                 operation_index, event_index, topics, data)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            ON CONFLICT (contract_id, ledger_sequence, operation_index, event_index) DO NOTHING
            "#,
            event.contract_id,
            event.contract_name,
            event.event_name,
            event.ledger_sequence,
            event.operation_index,
            event.event_index,
            event.topics,
            event.data,
        )
        .execute(pool)
        .await?;
    }
    Ok(())
}

/// Loads the persisted cursor for a contract, or the start position.
pub async fn get_cursor(pool: &PgPool, contract_id: &str) -> Result<Cursor, sqlx::Error> {
    let row = sqlx::query!(
        r#"
        SELECT ledger_sequence, operation_index, event_index
        FROM indexer.indexer_cursors
        WHERE contract_id = $1
        "#,
        contract_id
    )
    .fetch_optional(pool)
    .await?;

    Ok(match row {
        Some(row) => Cursor {
            ledger_sequence: row.ledger_sequence,
            operation_index: row.operation_index,
            event_index: row.event_index,
        },
        None => Cursor::START,
    })
}

/// Persists the resume cursor for a contract (upsert).
pub async fn set_cursor(
    pool: &PgPool,
    contract_id: &str,
    cursor: Cursor,
) -> Result<(), sqlx::Error> {
    sqlx::query!(
        r#"
        INSERT INTO indexer.indexer_cursors
            (contract_id, ledger_sequence, operation_index, event_index)
        VALUES ($1, $2, $3, $4)
        ON CONFLICT (contract_id) DO UPDATE SET
            ledger_sequence = EXCLUDED.ledger_sequence,
            operation_index = EXCLUDED.operation_index,
            event_index = EXCLUDED.event_index,
            updated_at = NOW()
        "#,
        contract_id,
        cursor.ledger_sequence,
        cursor.operation_index,
        cursor.event_index,
    )
    .execute(pool)
    .await?;
    Ok(())
}

/// Deletes raw events at or after `from_ledger` for a contract and resets the
/// cursor so ingestion resumes from there (re-org rewind).
pub async fn rewind(pool: &PgPool, contract_id: &str, from_ledger: i64) -> Result<(), sqlx::Error> {
    sqlx::query!(
        r#"
        DELETE FROM indexer.indexed_events
        WHERE contract_id = $1 AND ledger_sequence >= $2
        "#,
        contract_id,
        from_ledger
    )
    .execute(pool)
    .await?;

    sqlx::query!(
        r#"
        DELETE FROM indexer.indexer_cursors
        WHERE contract_id = $1
        "#,
        contract_id
    )
    .execute(pool)
    .await?;

    Ok(())
}

/// Loads stored events at or below the finality floor for a contract, ordered
/// by position. These are the events eligible for projection writes.
pub async fn finalized_events(
    pool: &PgPool,
    contract_id: &str,
    finality_floor: i64,
) -> Result<Vec<ChainEvent>, sqlx::Error> {
    let rows = sqlx::query!(
        r#"
        SELECT contract_id, contract_name, event_name, ledger_sequence,
               operation_index, event_index, topics, data
        FROM indexer.indexed_events
        WHERE contract_id = $1 AND ledger_sequence <= $2
        ORDER BY ledger_sequence, operation_index, event_index
        "#,
        contract_id,
        finality_floor
    )
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|row| ChainEvent {
            contract_id: row.contract_id,
            contract_name: row.contract_name,
            event_name: row.event_name,
            ledger_sequence: row.ledger_sequence,
            operation_index: row.operation_index,
            event_index: row.event_index,
            topics: row.topics,
            data: row.data,
        })
        .collect())
}
