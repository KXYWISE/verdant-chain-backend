use hex;
use sha2::{Digest, Sha256};

use super::model::FarmerMetadata;

fn canonical_bytes(metadata: &FarmerMetadata) -> Vec<u8> {
    let mut map = serde_json::Map::new();
    map.insert("name".into(), serde_json::json!(metadata.name));
    if let Some(region) = &metadata.region {
        map.insert("region".into(), serde_json::json!(region));
    }
    if let Some(district) = &metadata.district {
        map.insert("district".into(), serde_json::json!(district));
    }
    if let Some(bio) = &metadata.bio {
        map.insert("bio".into(), serde_json::json!(bio));
    }
    if let Some(hash) = &metadata.profile_image_hash {
        map.insert("profileImageHash".into(), serde_json::json!(hash));
    }
    serde_json::to_vec(&map).expect("canonical metadata serialization")
}

/// sha256 of the canonical serialization of the off-chain profile metadata
/// (AD-004, AD-009). Deterministic: keys are emitted in sorted order.
pub fn metadata_hash(metadata: &FarmerMetadata) -> String {
    let digest = Sha256::digest(canonical_bytes(metadata));
    hex::encode(digest)
}

#[cfg(test)]
mod tests {
    use crate::farmers::hash::metadata_hash;
    use crate::farmers::model::FarmerMetadata;

    fn sample() -> FarmerMetadata {
        FarmerMetadata {
            name: "Ada Farm Cooperative".into(),
            region: Some("Niger".into()),
            district: None,
            bio: None,
            profile_image_hash: None,
        }
    }

    #[test]
    fn hash_is_stable_and_hex() {
        let a = metadata_hash(&sample());
        let b = metadata_hash(&sample());
        assert_eq!(a, b);
        assert_eq!(a.len(), 64);
        assert!(a.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn optional_fields_change_the_hash() {
        let mut other = sample();
        other.bio = Some("Grows millet".into());
        assert_ne!(metadata_hash(&sample()), metadata_hash(&other));
    }
}
