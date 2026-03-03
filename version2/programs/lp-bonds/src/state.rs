use anchor_lang::prelude::*;

/// Protocol configuration account.
/// Stores global settings including allowlisted whirlpool and admin authority.
///
/// PDA Seeds: ["config"]
#[account]
#[derive(InitSpace)]
pub struct ProtocolConfig {
    /// Admin authority who can update config
    pub admin: Pubkey,

    /// The only whirlpool this protocol interacts with
    /// Hardcoded check + stored for runtime verification
    pub allowlisted_whirlpool: Pubkey,

    /// Counter for total bonds minted (used for tracking/analytics)
    pub bond_counter: u64,

    /// PDA bump seed
    pub bump: u8,
}

/// Position custody account.
/// Stores metadata about a custodied whirlpool position.
/// The associated position NFT is held in a token account owned by this PDA.
///
/// PDA Seeds: ["position_custody", bond_mint]
#[account]
#[derive(InitSpace)]
pub struct PositionCustody {
    /// Bond NFT mint that represents ownership
    pub bond_mint: Pubkey,

    /// Whirlpool position NFT mint
    pub position_mint: Pubkey,

    /// Whirlpool this position is in
    pub whirlpool: Pubkey,

    /// Lower tick index of position
    pub tick_lower_index: i32,

    /// Upper tick index of position
    pub tick_upper_index: i32,

    /// Initial liquidity deposited
    pub liquidity: u128,

    /// Original depositor address
    pub depositor: Pubkey,

    /// Unix timestamp when position was created
    pub created_at: i64,

    /// PDA bump seed for this custody account
    pub bump: u8,

    /// Position PDA bump (from whirlpool)
    pub position_bump: u8,
}

impl PositionCustody {
    /// Derives the position custody PDA address.
    pub fn derive_address(bond_mint: &Pubkey, program_id: &Pubkey) -> (Pubkey, u8) {
        Pubkey::find_program_address(
            &[b"position_custody", bond_mint.as_ref()],
            program_id,
        )
    }
}

/// Bond NFT metadata (off-chain representation).
/// Stored in Metaplex metadata account, not on-chain in this program.
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug)]
pub struct BondMetadata {
    /// Bond name
    pub name: String,
    /// Bond symbol
    pub symbol: String,
    /// Metadata URI
    pub uri: String,
    /// Associated whirlpool
    pub whirlpool: Pubkey,
    /// Position details
    pub position_mint: Pubkey,
    /// Tick range
    pub tick_lower: i32,
    pub tick_upper: i32,
}

// =============================================================================
// ORACLE STATE ACCOUNTS
// =============================================================================

/// Oracle configuration account.
/// Stores the trusted oracle authority for signature verification.
///
/// PDA Seeds: ["oracle_config"]
#[account]
#[derive(InitSpace)]
pub struct OracleConfig {
    /// The trusted oracle authority pubkey (Ed25519 public key)
    pub oracle_authority: Pubkey,

    /// Admin who can update oracle authority
    pub admin: Pubkey,

    /// Whether the oracle is enabled
    pub enabled: bool,

    /// PDA bump seed
    pub bump: u8,
}

impl OracleConfig {
    /// Derives the oracle config PDA address.
    pub fn derive_address(program_id: &Pubkey) -> (Pubkey, u8) {
        Pubkey::find_program_address(&[b"oracle_config"], program_id)
    }
}

/// Nonce account for replay protection.
/// Each user has their own nonce counter to prevent signature replay attacks.
///
/// PDA Seeds: ["nonce", user_pubkey]
#[account]
#[derive(InitSpace)]
pub struct NonceAccount {
    /// User this nonce belongs to
    pub user: Pubkey,

    /// Current nonce value (new nonces must be strictly greater)
    pub current_nonce: u64,

    /// PDA bump seed
    pub bump: u8,
}

impl NonceAccount {
    /// Derives the nonce account PDA address for a user.
    pub fn derive_address(user: &Pubkey, program_id: &Pubkey) -> (Pubkey, u8) {
        Pubkey::find_program_address(&[b"nonce", user.as_ref()], program_id)
    }
}
