use sha2::{Sha256, Digest};
use crate::state::types::AssetType;

/// Hash an AssetType for use in order key computation.
/// Equivalent to LibAsset.hash(AssetType) in Solidity.
pub fn hash_asset_type(asset_type: &AssetType) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(&[asset_type.asset_class.clone() as u8]);
    hasher.update(asset_type.mint.to_bytes());
    hasher.update(asset_type.token_id.to_le_bytes());
    hasher.finalize().into()
}
