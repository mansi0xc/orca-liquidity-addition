use anchor_lang::prelude::*;

/// Protocol configuration account.
/// Stores global settings including allowlisted whirlpool, token mints, and admin authority.
///
/// PDA Seeds: ["config"]
#[account]
#[derive(InitSpace)]
pub struct ProtocolConfig {
    /// Admin authority who can update config
    pub admin: Pubkey,

    /// Pending admin for two-step transfer (Pubkey::default() if none)
    pub pending_admin: Pubkey,

    /// The only whirlpool this protocol interacts with (Level 1)
    pub allowlisted_whirlpool: Pubkey,

    /// Token mint A for the configured Whirlpool pair
    pub token_mint_a: Pubkey,

    /// Token mint B for the configured Whirlpool pair
    pub token_mint_b: Pubkey,

    /// Enforced lower tick index for all Level 1 positions.
    /// All bonds in this protocol use the same tick range — prevents
    /// user-controlled tick manipulation and ensures uniform positions.
    pub tick_lower_index: i32,

    /// Enforced upper tick index for all Level 1 positions.
    pub tick_upper_index: i32,

    /// Lock duration in seconds for Level 1 bonds
    pub lock_duration: i64,

    /// Counter for total bonds minted
    pub bond_counter: u64,

    /// Whether protocol operations are paused
    pub is_paused: bool,

    /// PDA bump seed
    pub bump: u8,
}

/// Position custody account.
/// Stores metadata about a custodied Orca Whirlpool position.
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

    /// Orca Whirlpool this position is in
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

    /// Bond level (1-4)
    pub level: u8,

    /// Lock duration in seconds
    pub lock_duration: i64,

    /// Whether this bond was evolved from a previous level
    pub is_evolved: bool,

    /// Source bond mint if evolved (Pubkey::default() if not)
    pub evolved_from: Pubkey,

    /// PDA bump seed
    pub bump: u8,

    /// Position PDA bump
    pub position_bump: u8,
}

impl PositionCustody {
    pub fn derive_address(bond_mint: &Pubkey, program_id: &Pubkey) -> (Pubkey, u8) {
        Pubkey::find_program_address(
            &[b"position_custody", bond_mint.as_ref()],
            program_id,
        )
    }

    pub fn is_lock_expired(&self, current_time: i64) -> bool {
        current_time >= self.created_at.saturating_add(self.lock_duration)
    }

    pub fn unlock_time(&self) -> i64 {
        self.created_at.saturating_add(self.lock_duration)
    }
}

/// Oracle configuration account.
///
/// PDA Seeds: ["oracle_config"]
#[account]
#[derive(InitSpace)]
pub struct OracleConfig {
    /// The trusted oracle authority pubkey
    pub oracle_authority: Pubkey,

    /// Admin who can update oracle authority
    pub admin: Pubkey,

    /// Whether the oracle is enabled (toggleable via set_oracle_enabled)
    pub enabled: bool,

    /// PDA bump seed
    pub bump: u8,
}

impl OracleConfig {
    pub fn derive_address(program_id: &Pubkey) -> (Pubkey, u8) {
        Pubkey::find_program_address(&[b"oracle_config"], program_id)
    }
}

/// Nonce account for replay protection.
///
/// PDA Seeds: ["nonce", user_pubkey]
#[account]
#[derive(InitSpace)]
pub struct NonceAccount {
    /// User this nonce belongs to
    pub user: Pubkey,

    /// Current nonce value
    pub current_nonce: u64,

    /// PDA bump seed
    pub bump: u8,
}

impl NonceAccount {
    pub fn derive_address(user: &Pubkey, program_id: &Pubkey) -> (Pubkey, u8) {
        Pubkey::find_program_address(&[b"nonce", user.as_ref()], program_id)
    }
}

/// Exchange configuration account.
/// Stores the output token mint and active status for bond exchanges.
///
/// PDA Seeds: ["exchange_config"]
#[account]
#[derive(InitSpace)]
pub struct ExchangeConfig {
    /// The SPL token mint to pay out on exchange
    pub token_mint_out: Pubkey,

    /// Whether exchange is active
    pub is_active: bool,

    /// Admin who configured this
    pub admin: Pubkey,

    /// PDA bump seed
    pub bump: u8,
}

impl ExchangeConfig {
    pub fn derive_address(program_id: &Pubkey) -> (Pubkey, u8) {
        Pubkey::find_program_address(&[b"exchange_config"], program_id)
    }
}

/// Exchange nonce account — PDA-per-nonce replay protection.
/// If this PDA exists, the nonce has been used. Creating it marks the nonce as consumed.
///
/// PDA Seeds: ["exchange_nonce", user_pubkey, nonce_le_bytes]
#[account]
#[derive(InitSpace)]
pub struct ExchangeNonce {
    /// User who used this nonce
    pub user: Pubkey,

    /// The nonce value
    pub nonce: u64,

    /// PDA bump seed
    pub bump: u8,
}
