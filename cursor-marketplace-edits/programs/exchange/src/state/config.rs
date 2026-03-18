use anchor_lang::prelude::*;

#[account]
#[derive(InitSpace)]
pub struct ExchangeConfig {
    pub owner: Pubkey,
    pub exchange_owner: Pubkey,
    pub order_book: Pubkey,
    pub default_fee_receiver: Pubkey,
    pub royalties_registry_program: Pubkey,
    pub wsol_mint: Pubkey,
    pub protocol_fee_bps: u16,
    pub is_paused: bool,
    pub bump: u8,
}
