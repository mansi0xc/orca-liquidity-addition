use anchor_lang::prelude::*;
use solana_pubkey::pubkey;

// =============================================================================
// PROGRAM SEEDS
// =============================================================================

pub const CONFIG_SEED: &[u8] = b"config";
pub const BOND_AUTHORITY_SEED: &[u8] = b"bond_authority";
pub const POSITION_CUSTODY_SEED: &[u8] = b"position_custody";
pub const ORACLE_CONFIG_SEED: &[u8] = b"oracle_config";
pub const NONCE_SEED: &[u8] = b"nonce";
pub const EXCHANGE_CONFIG_SEED: &[u8] = b"exchange_config";
pub const EXCHANGE_MINT_AUTHORITY_SEED: &[u8] = b"exchange_mint_authority";
pub const EXCHANGE_NONCE_SEED: &[u8] = b"exchange_nonce";

// =============================================================================
// ORACLE CONSTANTS
// =============================================================================

/// Domain separator for mint-time oracle verification messages.
/// 18 bytes, padded to match ORACLE_DOMAIN_VERIFY length.
pub const ORACLE_DOMAIN_MINT: &[u8] = b"LP_BONDS_MINT_V1__";

/// Domain separator for post-mint collateral verification messages.
/// 18 bytes, must differ from ORACLE_DOMAIN_MINT to prevent cross-instruction replay.
pub const ORACLE_DOMAIN_VERIFY: &[u8] = b"LP_BONDS_VRFY_V1__";

/// Domain separator for exchange oracle messages.
/// 18 bytes, distinct from MINT/VERIFY to prevent cross-instruction replay.
pub const ORACLE_DOMAIN_EXCHANGE: &[u8] = b"LP_BONDS_XCHG_V1_";

/// Exchange canonical oracle message length: 130 bytes.
///
/// Layout (EVM parity — no timestamp):
///   domain(18) + bond_mint(32) + amount_out(8)
///   + nonce(8) + sender(32) + contract_address(32)
///   = 18 + 32 + 8 + 8 + 32 + 32 = 130
pub const EXCHANGE_MESSAGE_LEN: usize = 130;

/// Unified canonical oracle message length: 238 bytes.
///
/// Layout:
///   domain(18) + whirlpool(32) + token_mint_a(32) + token_mint_b(32)
///   + amount_a(8) + amount_b(8) + liquidity(16)
///   + tick_lower(4) + tick_upper(4) + tick_current(4)
///   + nonce(8) + timestamp(8) + sender(32) + contract_address(32)
pub const ORACLE_MESSAGE_LEN: usize = 238;

/// Maximum age of oracle timestamp before it's considered stale (seconds).
/// 60 seconds provides a reasonable window for transaction propagation
/// while preventing use of outdated market data.
pub const MAX_ORACLE_STALENESS_SECONDS: i64 = 60;

// =============================================================================
// SYSTEM CONSTANTS (immutable Solana values)
// =============================================================================

/// Native SOL mint address used by the SPL Token program
pub const NATIVE_MINT: Pubkey =
    pubkey!("So11111111111111111111111111111111111111112");

// =============================================================================
// TICK BOUNDS (Orca Whirlpool)
// =============================================================================

pub const MIN_TICK_INDEX: i32 = -443636;
pub const MAX_TICK_INDEX: i32 = 443636;

// =============================================================================
// BOND NFT DEFAULTS
// =============================================================================

pub const BOND_NFT_NAME_PREFIX: &str = "LP Bond #";
pub const BOND_NFT_SYMBOL: &str = "LPBOND";
pub const BOND_NFT_URI_BASE: &str = "https://api.lpbonds.io/metadata/";
