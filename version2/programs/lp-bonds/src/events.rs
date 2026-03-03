use anchor_lang::prelude::*;

/// Emitted when protocol is initialized.
#[event]
pub struct ProtocolInitialized {
    /// Admin authority pubkey
    pub admin: Pubkey,
    /// Allowlisted whirlpool address
    pub allowlisted_whirlpool: Pubkey,
}

/// Emitted when a bond NFT is minted after adding liquidity.
#[event]
pub struct BondMinted {
    /// Bond NFT mint address
    pub bond_mint: Pubkey,
    /// Whirlpool position NFT mint address
    pub position_mint: Pubkey,
    /// Whirlpool address
    pub whirlpool: Pubkey,
    /// User who deposited liquidity
    pub depositor: Pubkey,
    /// Lower tick index of position
    pub tick_lower_index: i32,
    /// Upper tick index of position
    pub tick_upper_index: i32,
    /// Liquidity amount deposited
    pub liquidity: u128,
    /// SOL amount wrapped and deposited
    pub sol_deposited: u64,
    /// Unix timestamp of mint
    pub timestamp: i64,
}

/// Emitted when a bond is redeemed.
#[event]
pub struct BondRedeemed {
    /// Bond NFT mint address (burned)
    pub bond_mint: Pubkey,
    /// Whirlpool position NFT mint address (transferred to user)
    pub position_mint: Pubkey,
    /// User who redeemed the bond
    pub redeemer: Pubkey,
    /// Unix timestamp of redemption
    pub timestamp: i64,
}

/// Emitted when protocol configuration is updated.
#[event]
pub struct ConfigUpdated {
    /// Previous admin
    pub old_admin: Pubkey,
    /// New admin
    pub new_admin: Pubkey,
    /// Timestamp of update
    pub timestamp: i64,
}

/// Emitted when liquidity is collected from a position.
#[event]
pub struct FeesCollected {
    /// Bond NFT mint
    pub bond_mint: Pubkey,
    /// Position mint
    pub position_mint: Pubkey,
    /// Token A fees collected
    pub fees_a: u64,
    /// Token B fees collected
    pub fees_b: u64,
    /// Collector address
    pub collector: Pubkey,
    /// Timestamp
    pub timestamp: i64,
}

// =============================================================================
// ORACLE EVENTS
// =============================================================================

/// Emitted when the oracle configuration is initialized.
#[event]
pub struct OracleInitialized {
    /// Oracle authority pubkey
    pub oracle_authority: Pubkey,
    /// Admin who initialized
    pub admin: Pubkey,
    /// Timestamp
    pub timestamp: i64,
}

/// Emitted when the oracle authority is updated.
#[event]
pub struct OracleAuthorityUpdated {
    /// Previous oracle authority
    pub old_authority: Pubkey,
    /// New oracle authority
    pub new_authority: Pubkey,
    /// Admin who updated
    pub admin: Pubkey,
    /// Timestamp
    pub timestamp: i64,
}

/// Emitted when collateral is successfully verified via oracle signature.
#[event]
pub struct CollateralVerified {
    /// Bond NFT mint
    pub bond_mint: Pubkey,
    /// Position NFT mint
    pub position_mint: Pubkey,
    /// User/sender who requested verification
    pub sender: Pubkey,
    /// Verified token A amount
    pub amount0: u64,
    /// Verified token B amount
    pub amount1: u64,
    /// Verified liquidity
    pub liquidity: u128,
    /// Nonce used (for replay tracking)
    pub nonce: u64,
    /// Oracle authority that signed
    pub oracle_authority: Pubkey,
    /// Timestamp
    pub timestamp: i64,
}

/// Emitted when a user's nonce is initialized.
#[event]
pub struct NonceInitialized {
    /// User pubkey
    pub user: Pubkey,
    /// Initial nonce value
    pub initial_nonce: u64,
    /// Timestamp
    pub timestamp: i64,
}

/// Emitted when a user's nonce is incremented.
#[event]
pub struct NonceIncremented {
    /// User pubkey
    pub user: Pubkey,
    /// Previous nonce value
    pub old_nonce: u64,
    /// New nonce value
    pub new_nonce: u64,
    /// Timestamp
    pub timestamp: i64,
}
