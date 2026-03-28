use anchor_lang::prelude::*;

/// Emitted when evolution system is initialized.
#[event]
pub struct EvolutionInitialized {
    pub admin: Pubkey,
    pub treasury: Pubkey,
    pub oracle_authority: Pubkey,
    pub lp_bonds_program_id: Pubkey,
    pub timestamp: i64,
}

/// Emitted when a level is configured.
#[event]
pub struct LevelConfigured {
    pub level_id: u8,
    pub whirlpool: Pubkey,
    pub token_mint_a: Pubkey,
    pub token_mint_b: Pubkey,
    pub layer_token_mint: Pubkey,
    pub required_amount_a: u64,
    pub required_amount_b: u64,
    pub fee_bps: u16,
    pub lock_duration: i64,
    pub is_active: bool,
    pub admin: Pubkey,
    pub timestamp: i64,
}

/// Emitted when a bond is evolved.
#[event]
pub struct BondEvolved {
    pub source_bond_mint: Pubkey,
    pub source_level: u8,
    pub target_bond_mint: Pubkey,
    pub target_level: u8,
    pub whirlpool: Pubkey,
    pub position_mint: Pubkey,
    pub evolver: Pubkey,
    pub amount_a: u64,
    pub amount_b: u64,
    pub liquidity: u128,
    pub fee_paid: u64,
    pub lock_duration: i64,
    pub timestamp: i64,
}

/// Emitted when evolution is paused.
#[event]
pub struct EvolutionPausedEvent {
    pub admin: Pubkey,
    pub timestamp: i64,
}

/// Emitted when evolution is unpaused.
#[event]
pub struct EvolutionUnpausedEvent {
    pub admin: Pubkey,
    pub timestamp: i64,
}

/// Emitted when treasury is updated.
#[event]
pub struct EvolutionTreasuryUpdated {
    pub old_treasury: Pubkey,
    pub new_treasury: Pubkey,
    pub admin: Pubkey,
    pub timestamp: i64,
}

/// Emitted when oracle is updated.
#[event]
pub struct EvolutionOracleUpdated {
    pub old_oracle: Pubkey,
    pub new_oracle: Pubkey,
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

/// Emitted when an authority is whitelisted.
#[event]
pub struct AuthorityAdded {
    pub authority: Pubkey,
    pub permissions: u8,
    pub added_by: Pubkey,
    pub timestamp: i64,
}

/// Emitted when an authority is removed.
#[event]
pub struct AuthorityRemoved {
    pub authority: Pubkey,
    pub removed_by: Pubkey,
    pub timestamp: i64,
}

/// Emitted when evolution nonce is initialized.
#[event]
pub struct EvolutionNonceInitialized {
    pub user: Pubkey,
    pub timestamp: i64,
}

/// Emitted when evolution nonce is incremented.
#[event]
pub struct EvolutionNonceIncremented {
    pub user: Pubkey,
    pub old_nonce: u64,
    pub new_nonce: u64,
    pub timestamp: i64,
}

/// Emitted when layer tokens are minted.
#[event]
pub struct LayerTokensMinted {
    pub layer_token_mint: Pubkey,
    pub recipient: Pubkey,
    pub amount: u64,
    pub level: u8,
    pub source_bond_mint: Pubkey,
    pub timestamp: i64,
}

/// Emitted when an evolved bond is redeemed.
#[event]
pub struct BondRedeemed {
    pub bond_mint: Pubkey,
    pub position_mint: Pubkey,
    pub redeemer: Pubkey,
    pub level: u8,
    pub timestamp: i64,
}

/// Emitted when fees are collected from a position.
#[event]
pub struct FeesCollected {
    pub bond_mint: Pubkey,
    pub position_mint: Pubkey,
    pub fees_a: u64,
    pub fees_b: u64,
    pub collector: Pubkey,
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
