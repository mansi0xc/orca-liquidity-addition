use anchor_lang::prelude::*;

/// Emitted when protocol is initialized.
#[event]
pub struct ProtocolInitialized {
    pub admin: Pubkey,
    pub allowlisted_whirlpool: Pubkey,
    pub token_mint_a: Pubkey,
    pub token_mint_b: Pubkey,
    pub lock_duration: i64,
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
    pub token_max_a: u64,
    pub token_max_b: u64,
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
    pub admin: Pubkey,
    pub allowlisted_whirlpool: Pubkey,
    pub token_mint_a: Pubkey,
    pub token_mint_b: Pubkey,
    pub lock_duration: i64,
    pub timestamp: i64,
}

/// Emitted when protocol is paused.
#[event]
pub struct ProtocolPausedEvent {
    pub admin: Pubkey,
    pub timestamp: i64,
}

/// Emitted when protocol is unpaused.
#[event]
pub struct ProtocolUnpausedEvent {
    pub admin: Pubkey,
    pub timestamp: i64,
}

/// Emitted when admin transfer is proposed.
#[event]
pub struct AdminTransferProposed {
    pub current_admin: Pubkey,
    pub pending_admin: Pubkey,
    pub timestamp: i64,
}

/// Emitted when admin transfer is accepted.
#[event]
pub struct AdminTransferAccepted {
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

/// Emitted when oracle verification succeeds during bond minting.
/// Provides an audit trail linking oracle-attested market data to the minted bond.
#[event]
pub struct OracleVerifiedForMint {
    pub bond_mint: Pubkey,
    pub whirlpool: Pubkey,
    pub sender: Pubkey,
    pub token_max_a: u64,
    pub token_max_b: u64,
    pub liquidity: u128,
    pub tick_lower: i32,
    pub tick_upper: i32,
    pub tick_current: i32,
    pub nonce: u64,
    pub oracle_timestamp: i64,
    pub oracle_authority: Pubkey,
    pub timestamp: i64,
}

/// Emitted when tokens are recovered by admin.
#[event]
pub struct RecoveryEvent {
    pub token_mint: Pubkey,
    pub amount: u64,
    pub admin: Pubkey,
    pub timestamp: i64,
}

/// Emitted when oracle enabled status changes.
#[event]
pub struct OracleEnabledChanged {
    pub enabled: bool,
    pub admin: Pubkey,
    pub timestamp: i64,
}

// =============================================================================
// EXCHANGE EVENTS
// =============================================================================

/// Emitted when exchange config is initialized.
#[event]
pub struct ExchangeConfigInitialized {
    pub token_mint_out: Pubkey,
    pub admin: Pubkey,
    pub timestamp: i64,
}

/// Emitted when exchange config is updated.
#[event]
pub struct ExchangeConfigUpdated {
    pub token_mint_out: Pubkey,
    pub is_active: bool,
    pub admin: Pubkey,
    pub timestamp: i64,
}

/// Emitted when a bond is exchanged for output tokens (EVM parity).
#[event]
pub struct BondExchanged {
    pub bond_mint: Pubkey,
    pub user: Pubkey,
    pub amount_out: u64,
}
