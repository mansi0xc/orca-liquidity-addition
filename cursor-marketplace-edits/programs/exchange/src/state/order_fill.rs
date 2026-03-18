use anchor_lang::prelude::*;

#[account]
#[derive(InitSpace)]
pub struct OrderFill {
    pub fill_amount: u64,
    pub bump: u8,
}
