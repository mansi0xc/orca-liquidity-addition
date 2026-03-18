use anchor_lang::prelude::*;

#[account]
#[derive(InitSpace)]
pub struct FeeReceiver {
    pub receiver: Pubkey,
    pub bump: u8,
}
