use std::collections::HashMap;

use async_trait::async_trait;
use serde_json::{Map as JsonMap, Value};
use stellar_rpc_client::{Client, EventStart, EventType};
use stellar_xdr::{Limits, PublicKey, ReadXdr, ScAddress, ScVal, Uint256};

use crate::indexer::chain::{ChainEvents, ChainEventsError};
use crate::indexer::model::{ChainEvent, Cursor};

/// Real Soroban RPC event source (Stellar Rust SDK `stellar-rpc-client`,
/// Agent #4 decision Q4, event-indexing plan §2).
///
/// Converts RPC events (base64 XDR `ScVal` topics/value) into the JSON shape
/// consumed by the raw store and projection builders: `event_name` comes from
/// `topic[0]` (a Symbol); the remaining topics and the value are decoded
/// recursively into JSON. `operation_index` is derived from the event's TOID so
/// the natural key `(contract_id, ledger_sequence, operation_index, event_index)`
/// stays ordered exactly as the chain emitted events.
#[derive(Debug, Clone)]
pub struct RpcEventSource {
    client: Client,
    /// Maps contract ID (strkey `C…`) to the indexer contract name
    /// (identity | verification | escrow | financing).
    contracts: HashMap<String, String>,
    /// Max events per `getEvents` page.
    page_limit: usize,
}

impl RpcEventSource {
    pub fn new(
        rpc_url: &str,
        contracts: HashMap<String, String>,
    ) -> Result<Self, ChainEventsError> {
        let client = Client::new_with_headers(rpc_url, http::HeaderMap::new())
            .map_err(|e| ChainEventsError::Unavailable(e.to_string()))?;
        Ok(Self {
            client,
            contracts,
            page_limit: 100,
        })
    }
}

#[async_trait]
impl ChainEvents for RpcEventSource {
    async fn events_after(
        &self,
        contract_id: &str,
        cursor: Cursor,
    ) -> Result<Vec<ChainEvent>, ChainEventsError> {
        let contract_name = self
            .contracts
            .get(contract_id)
            .cloned()
            .unwrap_or_else(|| "unknown".to_string());

        // Ledger 0 never exists; the START cursor resumes from the first ledger.
        let start = EventStart::Ledger(cursor.ledger_sequence.max(1) as u32);
        let contract_ids = [contract_id.to_string()];

        let mut decoded = Vec::new();
        let mut page_cursor: Option<String> = None;
        loop {
            let start = match &page_cursor {
                Some(c) => EventStart::Cursor(c.clone()),
                None => start.clone(),
            };
            let resp = self
                .client
                .get_events(
                    start,
                    Some(EventType::Contract),
                    &contract_ids,
                    &[],
                    Some(self.page_limit),
                )
                .await
                .map_err(|e| ChainEventsError::Internal(e.to_string()))?;

            for ev in &resp.events {
                let Some(chain) = decode_rpc_event(ev, &contract_name) else {
                    continue;
                };
                let pos = Cursor {
                    ledger_sequence: chain.ledger_sequence,
                    operation_index: chain.operation_index,
                    event_index: chain.event_index,
                };
                if pos.is_after(&cursor) {
                    decoded.push(chain);
                }
            }

            if resp.events.len() < self.page_limit {
                break;
            }
            page_cursor = Some(resp.cursor);
        }
        Ok(decoded)
    }

    async fn head_ledger(&self) -> Result<i64, ChainEventsError> {
        let latest = self
            .client
            .get_latest_ledger()
            .await
            .map_err(|e| ChainEventsError::Internal(e.to_string()))?;
        Ok(i64::from(latest.sequence))
    }
}

/// Decodes a raw RPC event into the JSON store shape. Returns `None` when the
/// topic layout is unexpected (e.g. missing event-name symbol).
fn decode_rpc_event(ev: &stellar_rpc_client::Event, contract_name: &str) -> Option<ChainEvent> {
    let event_scval = ScVal::from_xdr_base64(ev.topic.first()?, Limits::none()).ok()?;
    let event_name = symbol_to_string(&event_scval)?;

    let (toid, event_index) = ev.parse_cursor().ok()?;
    let mut decoded_topics = Vec::with_capacity(ev.topic.len().saturating_sub(1));
    for raw in ev.topic.iter().skip(1) {
        let scval = ScVal::from_xdr_base64(raw, Limits::none()).ok()?;
        decoded_topics.push(scval_to_json(&scval).ok()?);
    }
    let value_scval = ScVal::from_xdr_base64(&ev.value, Limits::none()).ok()?;
    let data = scval_to_json(&value_scval).ok()?;

    Some(ChainEvent {
        contract_id: ev.contract_id.clone(),
        contract_name: contract_name.to_string(),
        event_name,
        ledger_sequence: (toid >> 32) as i64,
        operation_index: (toid & 0xFFFF_FFFF) as i32,
        event_index,
        topics: Value::Array(decoded_topics),
        data,
    })
}

fn symbol_to_string(scval: &ScVal) -> Option<String> {
    match scval {
        ScVal::Symbol(s) => Some(s.0.to_string()),
        ScVal::String(s) => Some(s.0.to_string()),
        _ => None,
    }
}

fn scval_to_json(scval: &ScVal) -> Result<Value, ChainEventsError> {
    Ok(match scval {
        ScVal::Bool(b) => Value::Bool(*b),
        ScVal::Void => Value::Null,
        ScVal::Error(e) => Value::String(format!("{e:?}")),
        ScVal::U32(u) => Value::from(*u),
        ScVal::I32(i) => Value::from(*i),
        ScVal::U64(u) => Value::from(*u),
        ScVal::I64(i) => Value::from(*i),
        ScVal::Timepoint(t) => Value::from(t.0),
        ScVal::Duration(d) => Value::from(d.0),
        ScVal::U128(p) => {
            let v = (u128::from(p.hi) << 64) | u128::from(p.lo);
            match u64::try_from(v) {
                Ok(fit) => Value::from(fit),
                Err(_) => Value::String(v.to_string()),
            }
        }
        ScVal::I128(p) => i128_json(p.hi, p.lo),
        ScVal::U256(_) | ScVal::I256(_) => Value::Null,
        ScVal::Bytes(b) => Value::String(hex::encode(b.0.as_slice())),
        ScVal::String(s) => Value::String(s.0.to_string()),
        ScVal::Symbol(s) => Value::String(s.0.to_string()),
        ScVal::Vec(Some(v)) => Value::Array(
            v.0.iter()
                .map(scval_to_json)
                .collect::<Result<Vec<_>, _>>()?,
        ),
        ScVal::Vec(None) => Value::Null,
        ScVal::Map(Some(m)) => {
            let mut obj = JsonMap::new();
            for entry in m.0.iter() {
                let key = match &entry.key {
                    ScVal::Symbol(s) => s.0.to_string(),
                    ScVal::String(s) => s.0.to_string(),
                    other => {
                        return Err(ChainEventsError::Internal(format!(
                            "unexpected map key {other:?}"
                        )));
                    }
                };
                obj.insert(key, scval_to_json(&entry.val)?);
            }
            Value::Object(obj)
        }
        ScVal::Map(None) => Value::Null,
        ScVal::Address(a) => Value::String(address_strkey(a)),
        ScVal::ContractInstance(_)
        | ScVal::LedgerKeyContractInstance
        | ScVal::LedgerKeyNonce(_) => Value::Null,
    })
}

/// On-chain `i128` amounts are stored as `BIGINT` (i64) in projections (sqlx
/// 0.8 has no native i128); render a JSON number when it fits, else a string
/// to avoid silent precision loss.
fn i128_json(hi: i64, lo: u64) -> Value {
    let v = (i128::from(hi) << 64) | i128::from(lo);
    match i64::try_from(v) {
        Ok(fit) => Value::from(fit),
        Err(_) => Value::String(v.to_string()),
    }
}

fn address_strkey(addr: &ScAddress) -> String {
    use stellar_strkey::ed25519;
    match addr {
        ScAddress::Account(account) => {
            let PublicKey::PublicKeyTypeEd25519(Uint256(bytes)) = &account.0;
            format!(
                "{}",
                stellar_strkey::Strkey::PublicKeyEd25519(ed25519::PublicKey(*bytes))
            )
        }
        ScAddress::Contract(contract) => format!(
            "{}",
            stellar_strkey::Strkey::Contract(stellar_strkey::Contract(contract.0.0))
        ),
        ScAddress::MuxedAccount(m) => format!(
            "{}",
            stellar_strkey::Strkey::MuxedAccountEd25519(ed25519::MuxedAccount {
                ed25519: m.ed25519.0,
                id: m.id,
            })
        ),
        other => format!("{other:?}"),
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{scval_to_json, symbol_to_string};
    use crate::indexer::chain::ChainEvents;
    use crate::indexer::model::Cursor;

    use serde_json::Value;
    use stellar_xdr::{Limits, ReadXdr, ScVal};

    fn scval(symbol: &str) -> ScVal {
        ScVal::Symbol(symbol.to_string().try_into().unwrap())
    }

    #[test]
    fn decodes_scval_to_json() {
        assert_eq!(scval_to_json(&ScVal::U32(42)).unwrap(), json!(42));
        assert_eq!(
            scval_to_json(&ScVal::U64(1_000_000)).unwrap(),
            json!(1_000_000u64)
        );
        assert_eq!(scval_to_json(&ScVal::Bool(true)).unwrap(), json!(true));
        assert_eq!(scval_to_json(&ScVal::Void).unwrap(), Value::Null);

        let symbol = scval("VerificationCreated");
        assert_eq!(
            symbol_to_string(&symbol),
            Some("VerificationCreated".into())
        );
    }

    #[test]
    fn roundtrips_base64_xdr_scval() {
        // U64 42 as base64 XDR ScVal.
        let encoded = "AAAABQAAAAAAAAAq";
        let decoded = ScVal::from_xdr_base64(encoded, Limits::none()).unwrap();
        assert_eq!(decoded, ScVal::U64(42));
        assert_eq!(scval_to_json(&decoded).unwrap(), json!(42));
    }

    #[tokio::test]
    async fn stub_source_compatibility() {
        // The trait signature stays usable by both sources.
        use crate::indexer::chain::StubEventSource;
        let source = StubEventSource::new();
        let events = source.events_after("C1", Cursor::START).await.unwrap();
        assert!(events.is_empty());
    }

    #[test]
    fn decodes_rpc_event_to_chain_event() {
        use stellar_xdr::{Limits, ScVal, WriteXdr};

        use super::decode_rpc_event;
        use stellar_rpc_client::Event as RpcEvent;

        fn b64(scval: &ScVal) -> String {
            scval.to_xdr_base64(Limits::none()).unwrap()
        }

        // FinancingCreated: topics = [Symbol("FinancingCreated"), U64(7)];
        // value = [U64(7), Account(G...), ...] — simplified payload.
        #[allow(deprecated)]
        let event = RpcEvent {
            event_type: "contract".into(),
            ledger: 100,
            ledger_closed_at: "2026-08-19T00:00:00Z".into(),
            contract_id: "CDEBUG".into(),
            id: format!("{}-0", 100u64 << 32),
            operation_index: Some(0),
            transaction_index: Some(0),
            tx_hash: Some("deadbeef".into()),
            is_successful_contract_call: Some(true),
            topic: vec![
                b64(&ScVal::Symbol("FinancingCreated".try_into().unwrap())),
                b64(&ScVal::U64(7)),
            ],
            value: b64(&ScVal::Vec(Some(
                vec![
                    ScVal::U64(7),
                    ScVal::I128(stellar_xdr::Int128Parts {
                        hi: 0,
                        lo: 5_000_000_000,
                    }),
                    ScVal::Bool(false),
                ]
                .try_into()
                .unwrap(),
            ))),
        };

        let chain = decode_rpc_event(&event, "financing").unwrap();
        assert_eq!(chain.contract_id, "CDEBUG");
        assert_eq!(chain.contract_name, "financing");
        assert_eq!(chain.event_name, "FinancingCreated");
        assert_eq!(chain.ledger_sequence, 100);
        assert_eq!(chain.operation_index, 0);
        assert_eq!(chain.event_index, 0);
        assert_eq!(chain.topics, json!([7]));
        assert_eq!(chain.data, json!([7, 5_000_000_000i64, false]));
    }
}
