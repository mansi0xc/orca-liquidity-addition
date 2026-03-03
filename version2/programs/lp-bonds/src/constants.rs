use anchor_lang::prelude::*;
use solana_pubkey::pubkey;

// =============================================================================
// PROGRAM SEEDS
// =============================================================================

/// Seed for protocol configuration PDA
pub const CONFIG_SEED: &[u8] = b"config";

/// Seed for bond mint authority PDA
pub const BOND_AUTHORITY_SEED: &[u8] = b"bond_authority";

/// Seed for position custody PDA
pub const POSITION_CUSTODY_SEED: &[u8] = b"position_custody";

/// Seed for oracle configuration PDA
pub const ORACLE_CONFIG_SEED: &[u8] = b"oracle_config";

/// Seed for nonce account PDA (per-user replay protection)
pub const NONCE_SEED: &[u8] = b"nonce";

// =============================================================================
// ORACLE CONSTANTS
// =============================================================================

/// Domain separator for signature messages (18 bytes)
pub const SIGNATURE_DOMAIN: &[u8] = b"LP_BONDS_SOLANA_V1";

/// Length of the canonical message for oracle signatures
/// 18 (domain) + 32 (bond_mint) + 32 (position_mint) + 8 (amount0) + 8 (amount1)
/// + 16 (liquidity) + 4 (tick_lower) + 4 (tick_upper) + 4 (tick_current)
/// + 8 (nonce) + 32 (sender) + 32 (contract) = 198 bytes
pub const CANONICAL_MESSAGE_LEN: usize = 198;

// =============================================================================
// ALLOWLISTED WHIRLPOOL
// =============================================================================

/// The only whirlpool this protocol is authorized to interact with.
/// Hardcoded for security - prevents fake whirlpool injection.
pub const ALLOWLISTED_WHIRLPOOL: Pubkey =
    pubkey!("8gbgyrnZJKiiUT29SJJ3VeJ7x7zHy11exABgD3omwVmN");

// =============================================================================
// EXPECTED TOKEN MINTS
// =============================================================================

/// Native SOL mint (wSOL)
pub const NATIVE_MINT: Pubkey =
    pubkey!("So11111111111111111111111111111111111111112");

/// Expected token A for the whirlpool (wSOL)
/// This is the native SOL mint wrapped as SPL token
pub const EXPECTED_TOKEN_MINT_A: Pubkey = NATIVE_MINT;

/// Expected token B for the whirlpool
/// Mint: 4qbX8Mtx8XNt6DeCL414z67Dj9DJircMoSNEuX18AMB2
pub const EXPECTED_TOKEN_MINT_B: Pubkey =
    pubkey!("4qbX8Mtx8XNt6DeCL414z67Dj9DJircMoSNEuX18AMB2");

// =============================================================================
// TICK BOUNDS
// =============================================================================

/// Minimum tick index for Whirlpool positions
/// From Orca Whirlpool: MIN_TICK_INDEX = -443636
pub const MIN_TICK_INDEX: i32 = -443636;

/// Maximum tick index for Whirlpool positions
/// From Orca Whirlpool: MAX_TICK_INDEX = 443636
pub const MAX_TICK_INDEX: i32 = 443636;

// =============================================================================
// BOND NFT DEFAULTS
// =============================================================================

/// Default bond NFT name prefix
pub const BOND_NFT_NAME_PREFIX: &str = "LP Bond #";

/// Default bond NFT symbol
pub const BOND_NFT_SYMBOL: &str = "LPBOND";

/// Default bond NFT URI base
pub const BOND_NFT_URI_BASE: &str = "https://api.lpbonds.io/metadata/";
