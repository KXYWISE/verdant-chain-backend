use uuid::Uuid;

/// Backend-issued identifiers use UUIDv7 (time-ordered, index-friendly) per
/// AD-009. Contract-issued identifiers (verification/escrow/financing) are
/// counter-based and rendered at the boundary; this helper is for
/// backend-issued entities (asset, batch, equipment, livestock, booking).
pub fn uuid7() -> String {
    Uuid::now_v7().to_string()
}

pub fn document_id(content_hash: &str) -> String {
    format!("va:doc:{content_hash}")
}
