use anchor_lang::prelude::*;

#[event]
pub struct MatchEvent {
    pub left_order_key_hash: [u8; 32],
    pub right_order_key_hash: [u8; 32],
    pub left_maker: Pubkey,
    pub right_maker: Pubkey,
    pub new_left_fill: u64,
    pub new_right_fill: u64,
}

#[event]
pub struct CancelOrderEvent {
    pub order_key_hash: [u8; 32],
    pub maker: Pubkey,
}

#[event]
pub struct TransferEvent {
    pub asset_class: u8,
    pub from: Pubkey,
    pub to: Pubkey,
    pub mint: Pubkey,
    pub value: u64,
    pub transfer_direction: u8,
    pub transfer_type: u8,
}
