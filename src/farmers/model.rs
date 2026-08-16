use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::error::AppError;

pub const FARMER_ID_PREFIX: &str = "va:farmer:";

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct FarmerMetadata {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub region: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub district: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bio: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        rename = "profileImageHash"
    )]
    pub profile_image_hash: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct VerificationMarker {
    pub kind: String,
    pub issuer: String,
    #[serde(rename = "issuedLedger")]
    pub issued_ledger: i64,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct Farmer {
    pub address: String,
    pub id: String,
    pub registered: bool,
    #[serde(rename = "createdLedger")]
    pub created_ledger: Option<i64>,
    #[serde(rename = "updatedLedger")]
    pub updated_ledger: Option<i64>,
    pub metadata: FarmerMetadata,
    #[serde(rename = "metadataHash")]
    pub metadata_hash: String,
    #[serde(rename = "verificationMarkers")]
    pub verification_markers: Vec<VerificationMarker>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct RegisterFarmerRequest {
    pub address: String,
    pub metadata: FarmerMetadata,
    #[serde(default, rename = "metadataHash")]
    pub metadata_hash: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct UpdateMetadataRequest {
    pub metadata: FarmerMetadata,
}

pub fn farmer_id(address: &str) -> String {
    format!("{FARMER_ID_PREFIX}{address}")
}

/// Parses a farmer identifier: either a raw Stellar public key (`G…`) or the
/// presentation form `va:farmer:G…` (AD-009). Returns the canonical address.
pub fn parse_farmer_identifier(input: &str) -> Result<String, AppError> {
    let address = input.strip_prefix(FARMER_ID_PREFIX).unwrap_or(input);
    if is_valid_address(address) {
        Ok(address.to_string())
    } else {
        Err(AppError::BadRequest(format!(
            "invalid farmer address: {input}"
        )))
    }
}

fn is_valid_address(address: &str) -> bool {
    stellar_strkey::Strkey::from_string(address)
        .is_ok_and(|key| matches!(key, stellar_strkey::Strkey::PublicKeyEd25519(_)))
}

#[cfg(test)]
mod tests {
    use crate::farmers::model::{farmer_id, parse_farmer_identifier};

    const VALID: &str = "GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF";

    #[test]
    fn parses_raw_address() {
        assert_eq!(parse_farmer_identifier(VALID).unwrap(), VALID);
    }

    #[test]
    fn parses_presentation_form() {
        let input = farmer_id(VALID);
        assert_eq!(parse_farmer_identifier(&input).unwrap(), VALID);
    }

    #[test]
    fn renders_presentation_id() {
        assert_eq!(farmer_id(VALID), format!("va:farmer:{VALID}"));
    }

    #[test]
    fn rejects_invalid_address() {
        assert!(parse_farmer_identifier("not-a-key").is_err());
        assert!(parse_farmer_identifier("va:farmer:not-a-key").is_err());
        assert!(parse_farmer_identifier("").is_err());
    }
}
