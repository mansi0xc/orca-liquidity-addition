use anchor_lang::prelude::*;

/// Global operator registry state account.
/// Seeds: ["operator_registry"]
#[account]
pub struct OperatorRegistryState {
    /// Authority that can manage the registry
    pub authority: Pubkey,
    /// Address that receives revenue shares from collections
    pub fund_receiver: Pubkey,
    /// Revenue share in basis points (e.g., 500 = 5%)
    pub share_percentage_bps: u64,
    /// Whether the registry is paused
    pub paused: bool,
    /// PDA bump seed
    pub bump: u8,
}

impl OperatorRegistryState {
    pub const SEED_PREFIX: &'static [u8] = b"operator_registry";
    pub const SIZE: usize = 8 + 32 + 32 + 8 + 1 + 1; // discriminator + fields
}

/// Per-collection operator whitelist entry.
/// Seeds: ["operator_whitelist", collection, operator]
#[account]
pub struct OperatorWhitelist {
    /// The collection this whitelist applies to
    pub collection: Pubkey,
    /// The operator (marketplace) address
    pub operator: Pubkey,
    /// Whether the operator is allowed
    pub is_allowed: bool,
    /// PDA bump seed
    pub bump: u8,
}

impl OperatorWhitelist {
    pub const SEED_PREFIX: &'static [u8] = b"operator_whitelist";
    pub const SIZE: usize = 8 + 32 + 32 + 1 + 1;
}

/// Universal operator entry (allowed for all collections).
/// Seeds: ["universal_operator", operator]
#[account]
pub struct UniversalOperator {
    /// The operator (marketplace) address
    pub operator: Pubkey,
    /// Whether the operator is allowed
    pub is_allowed: bool,
    /// PDA bump seed
    pub bump: u8,
}

impl UniversalOperator {
    pub const SEED_PREFIX: &'static [u8] = b"universal_operator";
    pub const SIZE: usize = 8 + 32 + 1 + 1;
}
