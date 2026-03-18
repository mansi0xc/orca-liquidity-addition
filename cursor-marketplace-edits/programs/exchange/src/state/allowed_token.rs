use anchor_lang::prelude::*;

#[account]
#[derive(InitSpace)]
pub struct AllowedToken {
    pub is_allowed: bool,
    pub bump: u8,
}
