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
