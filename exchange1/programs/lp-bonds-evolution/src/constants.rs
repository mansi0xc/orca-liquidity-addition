// =============================================================================
// PROGRAM SEEDS
// =============================================================================

pub const EVOLUTION_CONFIG_SEED: &[u8] = b"evolution_config";
pub const LEVEL_CONFIG_SEED: &[u8] = b"level_config";
pub const EVOLUTION_RECORD_SEED: &[u8] = b"evolution_record";
pub const LAYER_TOKEN_AUTHORITY_SEED: &[u8] = b"layer_token_authority";
pub const BOND_AUTHORITY_SEED: &[u8] = b"bond_authority";
pub const POSITION_CUSTODY_SEED: &[u8] = b"position_custody";
pub const EVOLUTION_NONCE_SEED: &[u8] = b"evolution_nonce";
pub const AUTHORITY_WHITELIST_SEED: &[u8] = b"authority_whitelist";

// =============================================================================
// ORACLE CONSTANTS
// =============================================================================

pub const EVOLUTION_SIGNATURE_DOMAIN: &[u8] = b"LP_BONDS_EVOLVE_V1";

/// Evolution canonical oracle message length: 271 bytes.
///
/// Layout:
///   domain(18) + source_bond_mint(32) + target_level(1)
///   + whirlpool(32) + token_mint_a(32) + token_mint_b(32)
///   + amount_a(8) + amount_b(8) + liquidity(16)
///   + tick_lower(4) + tick_upper(4) + tick_current(4)
///   + nonce(8) + timestamp(8) + sender(32) + contract_address(32)
pub const EVOLUTION_CANONICAL_MESSAGE_LEN: usize = 271;

/// Maximum age of oracle timestamp before it's considered stale (seconds).
/// 60 seconds provides a reasonable window for transaction propagation
/// while preventing use of outdated market data.
pub const MAX_ORACLE_STALENESS_SECONDS: i64 = 60;

// =============================================================================
// TICK BOUNDS (Orca Whirlpool)
// =============================================================================

pub const MIN_TICK_INDEX: i32 = -443636;
pub const MAX_TICK_INDEX: i32 = 443636;

// =============================================================================
// EVOLUTION CONSTANTS
// =============================================================================

pub const MAX_BOND_LEVEL: u8 = 4;
pub const MIN_BOND_LEVEL: u8 = 1;
pub const FEE_BPS_DENOMINATOR: u64 = 10000;
pub const MAX_FEE_BPS: u16 = 5000;

// =============================================================================
// AUTHORITY PERMISSION BITMASK
// =============================================================================

pub const PERM_CONFIGURE_LEVELS: u8 = 0x01;
pub const PERM_PAUSE: u8 = 0x02;
pub const PERM_UPDATE_TREASURY: u8 = 0x04;
pub const PERM_UPDATE_ORACLE: u8 = 0x08;

// =============================================================================
// RATE LIMITING
// =============================================================================

/// Minimum delay (seconds) between successive evolve_bond calls per user.
pub const MIN_EVOLUTION_DELAY_SECONDS: i64 = 5;
