use anchor_lang::prelude::*;

#[account]
#[derive(InitSpace)]
pub struct RoyaltyProvider {
    pub provider_program: Pubkey,
    pub bump: u8,
}
