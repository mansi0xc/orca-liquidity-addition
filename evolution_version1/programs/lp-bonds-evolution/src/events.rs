use anchor_lang::prelude::*;

/// Emitted when evolution system is initialized.
#[event]
pub struct EvolutionInitialized {
    pub admin: Pubkey,
    pub treasury: Pubkey,
    pub oracle_authority: Pubkey,
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
pub struct EvolutionPaused {
    pub admin: Pubkey,
    pub timestamp: i64,
}

/// Emitted when evolution is unpaused.
#[event]
pub struct EvolutionUnpaused {
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

/// Emitted when nonce is incremented.
#[event]
pub struct NonceIncremented {
    pub user: Pubkey,
    pub old_nonce: u64,
    pub new_nonce: u64,
    pub timestamp: i64,
}
