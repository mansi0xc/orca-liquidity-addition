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

    /// Whether the oracle is enabled
    pub oracle_enabled: bool,

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

    /// Maximum total layer tokens that can be minted for this level (EBG-09: mint cap)
    pub max_total_mint: u64,

    /// Total layer tokens minted so far for this level
    pub total_minted: u64,

    /// Maximum layer tokens that can be minted in a single transaction
    pub max_mint_per_tx: u64,

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

    pub fn calculate_fee(&self, amount: u64) -> std::result::Result<u64, anchor_lang::error::Error> {
        let product = (amount as u128)
            .checked_mul(self.fee_bps as u128)
            .ok_or(error!(crate::errors::EvolutionError::ArithmeticOverflow))?;
        let fee = product
            .checked_div(10000)
            .ok_or(error!(crate::errors::EvolutionError::ArithmeticOverflow))?;
        // Enforce minimum fee of 1 when fee_bps > 0 and amount > 0.
        // Prevents rounding-to-zero bypass on small amounts.
        let fee = if fee == 0 && self.fee_bps > 0 && amount > 0 {
            1u128
        } else {
            fee
        };
        require!(fee <= u64::MAX as u128, crate::errors::EvolutionError::ArithmeticOverflow);
        Ok(fee as u64)
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

    /// Timestamp of last successful evolve_bond execution (rate limiting)
    pub last_execution_timestamp: i64,

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

#[cfg(test)]
mod tests {
    use super::*;

    fn make_level_config(fee_bps: u16) -> LevelConfig {
        LevelConfig {
            level_id: 2,
            whirlpool: Pubkey::default(),
            token_mint_a: Pubkey::default(),
            token_mint_b: Pubkey::default(),
            layer_token_mint: Pubkey::default(),
            tick_lower: -100,
            tick_upper: 100,
            required_amount_a: 0,
            required_amount_b: 0,
            fee_bps,
            lock_duration: 86400,
            multiplier: 100,
            is_active: true,
            max_total_mint: u64::MAX,
            total_minted: 0,
            max_mint_per_tx: u64::MAX,
            bump: 255,
        }
    }

    #[test]
    fn test_fee_minimum_one_for_small_amount() {
        // amount=1, fee_bps=1 → (1*1)/10000 = 0, but must return 1
        let config = make_level_config(1);
        let fee = config.calculate_fee(1).unwrap();
        assert_eq!(fee, 1, "fee must be at least 1 when fee_bps > 0 and amount > 0");
    }

    #[test]
    fn test_fee_normal_case() {
        // amount=10000, fee_bps=250 (2.5%) → (10000*250)/10000 = 250
        let config = make_level_config(250);
        let fee = config.calculate_fee(10000).unwrap();
        assert_eq!(fee, 250);
    }

    #[test]
    fn test_fee_zero_amount() {
        // amount=0, fee_bps=500 → fee must be 0 (no amount, no fee)
        let config = make_level_config(500);
        let fee = config.calculate_fee(0).unwrap();
        assert_eq!(fee, 0);
    }

    #[test]
    fn test_fee_zero_bps() {
        // amount=1000, fee_bps=0 → fee must be 0 (no fee configured)
        let config = make_level_config(0);
        let fee = config.calculate_fee(1000).unwrap();
        assert_eq!(fee, 0);
    }

    #[test]
    fn test_fee_just_above_rounding_threshold() {
        // amount=100, fee_bps=100 (1%) → (100*100)/10000 = 1
        // This should NOT trigger the minimum — natural result is already 1
        let config = make_level_config(100);
        let fee = config.calculate_fee(100).unwrap();
        assert_eq!(fee, 1);
    }

    #[test]
    fn test_fee_large_amount_no_overflow() {
        // amount=u64::MAX, fee_bps=5000 (50%) → must not overflow
        let config = make_level_config(5000);
        let fee = config.calculate_fee(u64::MAX).unwrap();
        let expected = ((u64::MAX as u128) * 5000u128) / 10000u128;
        assert_eq!(fee, expected as u64);
    }

    #[test]
    fn test_fee_minimum_applies_at_boundary() {
        // amount=99, fee_bps=100 (1%) → (99*100)/10000 = 0 → must return 1
        let config = make_level_config(100);
        let fee = config.calculate_fee(99).unwrap();
        assert_eq!(fee, 1, "fee must be 1 when rounding would produce 0");
    }
}
