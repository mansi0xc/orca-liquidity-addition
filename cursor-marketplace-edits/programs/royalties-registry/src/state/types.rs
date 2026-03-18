use anchor_lang::prelude::*;

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug)]
pub struct RoyaltyPart {
    pub account: Pubkey,
    pub value: u16,
}

pub const MAX_ROYALTY_RECIPIENTS: usize = 10;
