use anchor_lang::prelude::*;

/// Emitted when protocol is initialized.
#[event]
pub struct ProtocolInitialized {
    pub admin: Pubkey,
    pub allowlisted_whirlpool: Pubkey,
}

/// Emitted when a Level 1 bond NFT is minted.
#[event]
pub struct BondMinted {
    pub bond_mint: Pubkey,
    pub position_mint: Pubkey,
    pub whirlpool: Pubkey,
    pub depositor: Pubkey,
    pub tick_lower_index: i32,
    pub tick_upper_index: i32,
    pub liquidity: u128,
    pub sol_deposited: u64,
    pub level: u8,
    pub lock_duration: i64,
    pub timestamp: i64,
}

/// Emitted when a bond is redeemed.
#[event]
pub struct BondRedeemed {
    pub bond_mint: Pubkey,
    pub position_mint: Pubkey,
    pub redeemer: Pubkey,
    pub level: u8,
    pub timestamp: i64,
}

/// Emitted when protocol configuration is updated.
#[event]
pub struct ConfigUpdated {
    pub old_admin: Pubkey,
    pub new_admin: Pubkey,
    pub timestamp: i64,
}

/// Emitted when fees are collected.
#[event]
pub struct FeesCollected {
    pub bond_mint: Pubkey,
    pub position_mint: Pubkey,
    pub fees_a: u64,
    pub fees_b: u64,
    pub collector: Pubkey,
    pub timestamp: i64,
}

// =============================================================================
// ORACLE EVENTS
// =============================================================================

#[event]
pub struct OracleInitialized {
    pub oracle_authority: Pubkey,
    pub admin: Pubkey,
    pub timestamp: i64,
}

#[event]
pub struct OracleAuthorityUpdated {
    pub old_authority: Pubkey,
    pub new_authority: Pubkey,
    pub admin: Pubkey,
    pub timestamp: i64,
}

#[event]
pub struct CollateralVerified {
    pub bond_mint: Pubkey,
    pub position_mint: Pubkey,
    pub sender: Pubkey,
    pub amount0: u64,
    pub amount1: u64,
    pub liquidity: u128,
    pub nonce: u64,
    pub oracle_authority: Pubkey,
    pub timestamp: i64,
}

#[event]
pub struct NonceInitialized {
    pub user: Pubkey,
    pub initial_nonce: u64,
    pub timestamp: i64,
}

#[event]
pub struct NonceIncremented {
    pub user: Pubkey,
    pub old_nonce: u64,
    pub new_nonce: u64,
    pub timestamp: i64,
}
