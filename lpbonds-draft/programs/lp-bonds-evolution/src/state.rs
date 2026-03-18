use anchor_lang::prelude::*;

/// Evolution configuration account.
///
/// PDA Seeds: ["evolution_config"]
#[account]
#[derive(InitSpace)]
pub struct EvolutionConfig {
    /// Admin authority
    pub admin: Pubkey,

    /// Pending admin for two-step transfer (Pubkey::default() if none)
    pub pending_admin: Pubkey,

    /// Treasury address for protocol fees
    pub treasury: Pubkey,

    /// Oracle authority for signature verification
    pub oracle_authority: Pubkey,

    /// LP Bonds base program ID (configurable, not hardcoded)
    pub lp_bonds_program_id: Pubkey,

    /// Whether evolution is paused
    pub is_paused: bool,

    /// Total evolutions performed
    pub evolution_counter: u64,

    /// PDA bump seed
    pub bump: u8,
}

impl EvolutionConfig {
    pub fn derive_address(program_id: &Pubkey) -> (Pubkey, u8) {
        Pubkey::find_program_address(&[b"evolution_config"], program_id)
    }
}

/// Level configuration account.
///
/// PDA Seeds: ["level_config", level_id]
#[account]
#[derive(InitSpace)]
pub struct LevelConfig {
    /// Level ID (2, 3, or 4)
    pub level_id: u8,

    /// Orca Whirlpool address for this level
    pub whirlpool: Pubkey,

    /// Token A mint
    pub token_mint_a: Pubkey,

    /// Token B mint  
    pub token_mint_b: Pubkey,

    /// Layer token mint
    pub layer_token_mint: Pubkey,

    /// Lower tick index
    pub tick_lower: i32,

    /// Upper tick index
    pub tick_upper: i32,

    /// Required amount of token A
    pub required_amount_a: u64,

    /// Required amount of token B
    pub required_amount_b: u64,

    /// Fee in basis points
    pub fee_bps: u16,

    /// Lock duration in seconds
    pub lock_duration: i64,

    /// Reward multiplier (100 = 1x)
    pub multiplier: u16,

    /// Whether level is active
    pub is_active: bool,

    /// PDA bump seed
    pub bump: u8,
}

impl LevelConfig {
    pub fn derive_address(level_id: u8, program_id: &Pubkey) -> (Pubkey, u8) {
        Pubkey::find_program_address(
            &[b"level_config", &[level_id]],
            program_id,
        )
    }

    pub fn calculate_fee(&self, amount: u64) -> u64 {
        (amount as u128)
            .checked_mul(self.fee_bps as u128)
            .unwrap_or(0)
            .checked_div(10000)
            .unwrap_or(0) as u64
    }
}

/// Evolution record account.
///
/// PDA Seeds: ["evolution_record", source_bond_mint]
#[account]
#[derive(InitSpace)]
pub struct EvolutionRecord {
    /// Source bond mint that was burned
    pub source_bond_mint: Pubkey,

    /// Source bond level
    pub source_level: u8,

    /// Target bond mint created
    pub target_bond_mint: Pubkey,

    /// Target bond level
    pub target_level: u8,

    /// User who evolved
    pub evolver: Pubkey,

    /// Timestamp
    pub evolved_at: i64,

    /// Token A amount
    pub amount_a: u64,

    /// Token B amount
    pub amount_b: u64,

    /// Liquidity added
    pub liquidity: u128,

    /// Fee paid
    pub fee_paid: u64,

    /// PDA bump
    pub bump: u8,
}

impl EvolutionRecord {
    pub fn derive_address(source_bond_mint: &Pubkey, program_id: &Pubkey) -> (Pubkey, u8) {
        Pubkey::find_program_address(
            &[b"evolution_record", source_bond_mint.as_ref()],
            program_id,
        )
    }
}

/// Layer token authority PDA.
///
/// PDA Seeds: ["layer_token_authority"]
#[account]
#[derive(InitSpace)]
pub struct LayerTokenAuthority {
    pub bump: u8,
}

impl LayerTokenAuthority {
    pub fn derive_address(program_id: &Pubkey) -> (Pubkey, u8) {
        Pubkey::find_program_address(&[b"layer_token_authority"], program_id)
    }
}

/// Evolution-specific nonce for replay protection.
/// Each user has their own nonce account.
///
/// PDA Seeds: ["evolution_nonce", user_pubkey]
#[account]
#[derive(InitSpace)]
pub struct EvolutionNonce {
    /// User this nonce belongs to
    pub user: Pubkey,

    /// Current nonce value
    pub current_nonce: u64,

    /// PDA bump seed
    pub bump: u8,
}

impl EvolutionNonce {
    pub fn derive_address(user: &Pubkey, program_id: &Pubkey) -> (Pubkey, u8) {
        Pubkey::find_program_address(
            &[b"evolution_nonce", user.as_ref()],
            program_id,
        )
    }
}

/// Authority whitelist for delegated admin permissions.
///
/// PDA Seeds: ["authority_whitelist", authority_pubkey]
#[account]
#[derive(InitSpace)]
pub struct AuthorityWhitelist {
    /// The whitelisted authority pubkey
    pub authority: Pubkey,

    /// Permission bitmask (PERM_CONFIGURE_LEVELS, PERM_PAUSE, etc.)
    pub permissions: u8,

    /// Admin who added this authority
    pub added_by: Pubkey,

    /// PDA bump seed
    pub bump: u8,
}

impl AuthorityWhitelist {
    pub fn derive_address(authority: &Pubkey, program_id: &Pubkey) -> (Pubkey, u8) {
        Pubkey::find_program_address(
            &[b"authority_whitelist", authority.as_ref()],
            program_id,
        )
    }
}

/// Position custody reference (read from base program).
/// This mirrors the PositionCustody from lp-bonds for deserialization.
#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct PositionCustodyRef {
    pub bond_mint: Pubkey,
    pub position_mint: Pubkey,
    pub whirlpool: Pubkey,
    pub tick_lower_index: i32,
    pub tick_upper_index: i32,
    pub liquidity: u128,
    pub depositor: Pubkey,
    pub created_at: i64,
    pub level: u8,
    pub lock_duration: i64,
    pub is_evolved: bool,
    pub evolved_from: Pubkey,
    pub bump: u8,
    pub position_bump: u8,
}

/// Nonce account reference (read from base program).
#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct NonceAccountRef {
    pub user: Pubkey,
    pub current_nonce: u64,
    pub bump: u8,
}
