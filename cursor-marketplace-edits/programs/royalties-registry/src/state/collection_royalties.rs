use anchor_lang::prelude::*;
use crate::state::types::{RoyaltyPart, MAX_ROYALTY_RECIPIENTS};

#[account]
pub struct CollectionRoyalties {
    pub initialized: bool,
    pub royalties: Vec<RoyaltyPart>,
    pub bump: u8,
}

impl CollectionRoyalties {
    pub const INIT_SPACE: usize =
        1  // initialized
        + 4 + (MAX_ROYALTY_RECIPIENTS * (32 + 2))  // Vec<RoyaltyPart>: 4 bytes len + max entries
        + 1; // bump
}
