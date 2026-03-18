use anchor_lang::prelude::*;

use crate::errors::ExchangeError;
use crate::state::types::*;

/// Equivalent to LibOrderData.parse.
/// Decodes order data into DataV1 { payouts, origin_fees }.
/// If payouts are empty, defaults to 100% payout to order maker.
pub fn parse_order_data(order: &Order) -> Result<DataV1> {
    if order.data_type == DATA_TYPE_V1 {
        let mut data: DataV1 = AnchorDeserialize::deserialize(&mut &order.data[..])
            .map_err(|_| ExchangeError::InvalidOrderDataType)?;

        if data.payouts.is_empty() {
            data.payouts = vec![Part {
                account: order.maker,
                value: 10000,
            }];
        }
        Ok(data)
    } else if order.data_type == DATA_TYPE_EMPTY {
        Ok(DataV1 {
            payouts: vec![Part {
                account: order.maker,
                value: 10000,
            }],
            origin_fees: vec![],
        })
    } else {
        Err(ExchangeError::InvalidOrderDataType.into())
    }
}
