use anchor_lang::prelude::*;
use sha2::{Sha256, Digest};

use crate::errors::ExchangeError;
use crate::state::types::*;
use crate::logic::asset::hash_asset_type;
use crate::logic::math::safe_get_partial_amount_floor;

/// Equivalent to LibOrder.hashKey: deterministic unique order identifier.
/// RULE R-2: includes maker, make_asset_type_hash, take_asset_type_hash, salt, collection_bid.
pub fn compute_order_key_hash(order: &Order) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(order.maker.to_bytes());
    hasher.update(hash_asset_type(&order.make_asset.asset_type));
    hasher.update(hash_asset_type(&order.take_asset.asset_type));
    hasher.update(order.salt.to_le_bytes());
    hasher.update([order.collection_bid as u8]);
    hasher.finalize().into()
}

/// Compute the full order hash for signature verification.
/// This is signed by the order maker.
/// RULE R-1: includes program_id and cluster byte.
pub fn compute_order_hash(order: &Order, program_id: &Pubkey) -> [u8; 32] {
    let mut hasher = Sha256::new();
    // Domain prefix (RULE R-1)
    hasher.update(program_id.to_bytes());
    hasher.update(b"energi");
    hasher.update([1u8]); // version
    // Order data
    hasher.update(order.maker.to_bytes());
    hasher.update(hash_asset_type(&order.make_asset.asset_type));
    hasher.update(order.make_asset.value.to_le_bytes());
    hasher.update(order.taker.to_bytes());
    hasher.update(hash_asset_type(&order.take_asset.asset_type));
    hasher.update(order.take_asset.value.to_le_bytes());
    hasher.update(order.salt.to_le_bytes());
    hasher.update(order.start.to_le_bytes());
    hasher.update(order.end.to_le_bytes());
    hasher.update(order.data_type);
    let data_hash: [u8; 32] = {
        let mut dh = Sha256::new();
        dh.update(&order.data);
        dh.finalize().into()
    };
    hasher.update(data_hash);
    hasher.update([order.collection_bid as u8]);
    hasher.finalize().into()
}

/// Compute the matchAllowance hash for Order Book signature verification.
/// RULE R-1: includes program_id.
pub fn compute_match_allowance_hash(
    order_key_hash: &[u8; 32],
    match_before_timestamp: i64,
    program_id: &Pubkey,
) -> [u8; 32] {
    let mut hasher = Sha256::new();
    // Domain prefix
    hasher.update(program_id.to_bytes());
    hasher.update(b"energi");
    hasher.update([1u8]); // version
    // Match allowance data
    hasher.update(order_key_hash);
    hasher.update(match_before_timestamp.to_le_bytes());
    hasher.finalize().into()
}

/// Equivalent to LibOrder.calculateRemaining.
/// Returns (make_value, take_value) remaining for this order given current fill.
pub fn calculate_remaining(order: &Order, take_asset_fill: u64) -> Result<(u64, u64)> {
    // RULE FM-2: check not cancelled
    require!(take_asset_fill < u64::MAX, ExchangeError::OrderCancelled);

    let take_value = order
        .take_asset
        .value
        .checked_sub(take_asset_fill)
        .ok_or(ExchangeError::ArithmeticOverflow)?;

    let make_value = safe_get_partial_amount_floor(
        order.make_asset.value,
        order.take_asset.value,
        take_value,
    )?;

    Ok((make_value, take_value))
}

/// Equivalent to LibOrder.validate: checks time constraints and asset class compatibility.
pub fn validate_order(order: &Order, current_timestamp: i64) -> Result<()> {
    if order.start != 0 {
        require!(
            order.start < current_timestamp,
            ExchangeError::OrderNotStarted
        );
    }
    if order.end != 0 {
        require!(
            order.end > current_timestamp,
            ExchangeError::OrderExpired
        );
    }

    validate_asset_classes(order)?;
    Ok(())
}

fn validate_asset_classes(order: &Order) -> Result<()> {
    let make_class = &order.make_asset.asset_type.asset_class;
    let take_class = &order.take_asset.asset_type.asset_class;

    let make_is_fungible = matches!(
        make_class,
        AssetClass::Sol | AssetClass::WrappedSol | AssetClass::SplToken
    );
    let make_is_non_fungible = matches!(
        make_class,
        AssetClass::Nft | AssetClass::SemiFungible
    );
    let take_is_fungible = matches!(
        take_class,
        AssetClass::Sol | AssetClass::WrappedSol | AssetClass::SplToken
    );
    let take_is_non_fungible = matches!(
        take_class,
        AssetClass::Nft | AssetClass::SemiFungible
    );

    if make_is_fungible {
        require!(take_is_non_fungible, ExchangeError::AssetClassMismatch);
    }
    if take_is_fungible {
        require!(make_is_non_fungible, ExchangeError::AssetClassMismatch);
    }
    if make_is_non_fungible {
        require!(take_is_fungible, ExchangeError::AssetClassMismatch);
    }

    Ok(())
}

/// Validate a collection bid maker order.
pub fn validate_collection_bid_maker_order(order: &Order) -> Result<()> {
    require!(order.collection_bid, ExchangeError::InvalidCollectionBidAsset);
    require!(order.salt > 0, ExchangeError::ZeroSaltCannotCancel);

    let make_class = &order.make_asset.asset_type.asset_class;
    require!(
        matches!(make_class, AssetClass::WrappedSol | AssetClass::SplToken),
        ExchangeError::InvalidCollectionBidAsset
    );

    let take_class = &order.take_asset.asset_type.asset_class;
    require!(
        matches!(take_class, AssetClass::Nft | AssetClass::SemiFungible),
        ExchangeError::InvalidCollectionBidAsset
    );

    Ok(())
}

/// Validate a collection bid taker order.
pub fn validate_collection_bid_taker_order(
    taker_order: &Order,
    maker_take_asset_mint: &Pubkey,
) -> Result<()> {
    require!(
        !taker_order.collection_bid,
        ExchangeError::InvalidCollectionBidTakerFlag
    );

    require!(
        taker_order.make_asset.asset_type.mint == *maker_take_asset_mint,
        ExchangeError::CollectionBidTakerAssetMismatch
    );

    Ok(())
}
