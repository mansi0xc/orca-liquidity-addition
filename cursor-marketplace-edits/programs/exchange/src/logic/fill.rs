use anchor_lang::prelude::*;

use crate::errors::ExchangeError;
use crate::state::types::*;
use crate::logic::math::safe_get_partial_amount_floor;
use crate::logic::order::calculate_remaining;

/// Equivalent to LibFill.fillOrder.
/// Returns FillResult with the take values for both sides.
pub fn fill_order(
    left_order: &Order,
    right_order: &Order,
    left_order_take_asset_fill: u64,
    right_order_take_asset_fill: u64,
) -> Result<FillResult> {
    let (left_make_value, left_take_value) =
        calculate_remaining(left_order, left_order_take_asset_fill)?;
    let (right_make_value, right_take_value) =
        calculate_remaining(right_order, right_order_take_asset_fill)?;

    if right_take_value > left_make_value {
        // Left order will be fully filled
        fill_left(
            left_make_value,
            left_take_value,
            right_order.make_asset.value,
            right_order.take_asset.value,
        )
    } else {
        // Right order will be fully filled (or both)
        fill_right(
            left_order.make_asset.value,
            left_order.take_asset.value,
            right_make_value,
            right_take_value,
        )
    }
}

/// Left order fully filled case.
/// Equivalent to LibFill.fillLeft.
fn fill_left(
    left_make_value: u64,
    left_take_value: u64,
    right_make_value: u64,
    right_take_value: u64,
) -> Result<FillResult> {
    let right_take = safe_get_partial_amount_floor(
        left_take_value,
        right_make_value,
        right_take_value,
    )?;

    require!(right_take <= left_make_value, ExchangeError::FillUnableToComplete);

    Ok(FillResult {
        right_order_take_value: right_take,
        left_order_take_value: left_take_value,
    })
}

/// Right order fully filled case.
/// Equivalent to LibFill.fillRight.
fn fill_right(
    left_make_value: u64,
    left_take_value: u64,
    right_make_value: u64,
    right_take_value: u64,
) -> Result<FillResult> {
    let left_take = safe_get_partial_amount_floor(
        right_take_value,
        left_make_value,
        left_take_value,
    )?;

    require!(left_take <= right_make_value, ExchangeError::FillUnableToComplete);

    Ok(FillResult {
        right_order_take_value: right_take_value,
        left_order_take_value: left_take,
    })
}
