use anchor_lang::prelude::*;

use crate::errors::ExchangeError;
use crate::state::types::*;
use crate::logic::bps;

/// Equivalent to LibExchange.matchAssets (single pair).
/// Verifies two asset types are compatible.
fn match_asset_types(
    take_asset_type: &AssetType,
    make_asset_type: &AssetType,
) -> Result<AssetType> {
    let class_take = &take_asset_type.asset_class;
    let class_make = &make_asset_type.asset_class;

    // SOL and wSOL are mutually compatible
    if matches!(class_take, AssetClass::Sol | AssetClass::WrappedSol)
        && matches!(class_make, AssetClass::Sol | AssetClass::WrappedSol)
    {
        return Ok(take_asset_type.clone());
    }

    // Same class: check data equality
    if class_take == class_make {
        if take_asset_type.mint == make_asset_type.mint
            && take_asset_type.token_id == make_asset_type.token_id
        {
            return Ok(take_asset_type.clone());
        }
    }

    Err(ExchangeError::AssetsDoNotMatch.into())
}

/// Equivalent to LibExchange.matchAssets (order pair).
/// Returns (maker_asset_type, taker_asset_type).
pub fn match_order_assets(
    order_left: &Order,
    order_right: &Order,
) -> Result<(AssetType, AssetType)> {
    let maker_asset_type = match_asset_types(
        &order_right.take_asset.asset_type,
        &order_left.make_asset.asset_type,
    )?;
    let taker_asset_type = match_asset_types(
        &order_left.take_asset.asset_type,
        &order_right.make_asset.asset_type,
    )?;

    Ok((maker_asset_type, taker_asset_type))
}

/// Equivalent to LibExchange.subFee: cap fee at value.
pub fn sub_fee(value: u64, fee: u64) -> (u64, u64) {
    if value > fee {
        (value - fee, fee)
    } else {
        (0, value)
    }
}

/// Equivalent to LibExchange.subFeeInBps.
/// Subtracts a fee (expressed as bps of total) from rest.
pub fn sub_fee_in_bps(rest: u64, total: u64, fee_in_bps: u16) -> Result<(u64, u64)> {
    let fee = bps::bps(total, fee_in_bps)?;
    Ok(sub_fee(rest, fee))
}

/// Equivalent to LibExchange.calculateTotalAmount.
/// Adds origin fees to the base amount.
pub fn calculate_total_amount(amount: u64, origin_fees: &[Part]) -> Result<u64> {
    let mut total = amount as u128;
    for fee in origin_fees {
        total = total
            .checked_add(bps::bps(amount, fee.value)? as u128)
            .ok_or(ExchangeError::ArithmeticOverflow)?;
    }
    Ok(total as u64)
}

/// Equivalent to LibExchange.checkCounterparties.
pub fn check_counterparties(order_left: &Order, order_right: &Order) -> Result<()> {
    if order_left.taker != Pubkey::default() {
        require!(
            order_right.maker == order_left.taker,
            ExchangeError::CounterpartyMismatch
        );
    }
    if order_right.taker != Pubkey::default() {
        require!(
            order_right.taker == order_left.maker,
            ExchangeError::CounterpartyMismatch
        );
    }
    Ok(())
}
