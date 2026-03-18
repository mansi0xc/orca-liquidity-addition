use anchor_lang::prelude::*;
use crate::state::types::{RoyaltyPart, MAX_ROYALTY_RECIPIENTS};

/// Owner-set royalties for a specific collection + tokenId combination.
#[account]
pub struct OwnerTokenRoyalties {
    pub initialized: bool,
    pub royalties: Vec<RoyaltyPart>,
    pub bump: u8,
}

impl OwnerTokenRoyalties {
    pub const INIT_SPACE: usize =
        1  // initialized
        + 4 + (MAX_ROYALTY_RECIPIENTS * (32 + 2))
        + 1; // bump
}

/// Creator-set royalties for a specific collection + tokenId combination.
#[account]
pub struct CreatorTokenRoyalties {
    pub initialized: bool,
    pub royalties: Vec<RoyaltyPart>,
    pub bump: u8,
}

impl CreatorTokenRoyalties {
    pub const INIT_SPACE: usize =
        1  // initialized
        + 4 + (MAX_ROYALTY_RECIPIENTS * (32 + 2))
        + 1; // bump
}
