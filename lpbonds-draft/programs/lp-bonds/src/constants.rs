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

// =============================================================================
// ORACLE CONSTANTS
// =============================================================================

pub const SIGNATURE_DOMAIN: &[u8] = b"LP_BONDS_SOLANA_V1";

/// Canonical message length: 198 bytes
pub const CANONICAL_MESSAGE_LEN: usize = 198;

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
