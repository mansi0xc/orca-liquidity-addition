use anchor_lang::prelude::*;

#[account]
#[derive(InitSpace)]
pub struct RegistryConfig {
    pub owner: Pubkey,
    pub bump: u8,
}
