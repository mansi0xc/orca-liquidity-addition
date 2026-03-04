use anchor_lang::prelude::*;
use solana_pubkey::pubkey;

// =============================================================================
// PROGRAM SEEDS
// =============================================================================

pub const EVOLUTION_CONFIG_SEED: &[u8] = b"evolution_config";
pub const LEVEL_CONFIG_SEED: &[u8] = b"level_config";
pub const EVOLUTION_RECORD_SEED: &[u8] = b"evolution_record";
pub const LAYER_TOKEN_AUTHORITY_SEED: &[u8] = b"layer_token_authority";
pub const BOND_AUTHORITY_SEED: &[u8] = b"bond_authority";
pub const POSITION_CUSTODY_SEED: &[u8] = b"position_custody";
pub const NONCE_SEED: &[u8] = b"nonce";

// =============================================================================
// LP BONDS BASE PROGRAM
// =============================================================================

/// LP Bonds base program ID for CPI and PDA verification
pub const LP_BONDS_PROGRAM_ID: Pubkey =
    pubkey!("AmJcNFdgckd1o6DPa6j12WGM6wNKZdvdWphtsP2Ws92w");

// =============================================================================
// ORACLE CONSTANTS
// =============================================================================

pub const EVOLUTION_SIGNATURE_DOMAIN: &[u8] = b"LP_BONDS_EVOLVE_V1";
pub const EVOLUTION_CANONICAL_MESSAGE_LEN: usize = 155;

// =============================================================================
// LEVEL 1 - ORCA WHIRLPOOL (SOL / Token)
// =============================================================================

pub const LEVEL_1_WHIRLPOOL: Pubkey =
    pubkey!("8gbgyrnZJKiiUT29SJJ3VeJ7x7zHy11exABgD3omwVmN");

pub const LEVEL_1_TOKEN_A: Pubkey =
    pubkey!("So11111111111111111111111111111111111111112");

pub const LEVEL_1_TOKEN_B: Pubkey =
    pubkey!("4qbX8Mtx8XNt6DeCL414z67Dj9DJircMoSNEuX18AMB2");

// =============================================================================
// LEVEL 2 - ORCA WHIRLPOOL
// =============================================================================

pub const LEVEL_2_WHIRLPOOL: Pubkey =
    pubkey!("36whP2YDjunT6VNCCPEn1MV9BrZxc5XsD7tAJMVahr1V");

pub const LEVEL_2_TOKEN_A: Pubkey =
    pubkey!("4qbX8Mtx8XNt6DeCL414z67Dj9DJircMoSNEuX18AMB2");

pub const LEVEL_2_TOKEN_B: Pubkey =
    pubkey!("Ci3iuaCJfQAapWHJkfycuTc67SCEZYfKTS8fxjKCP5tB");

// =============================================================================
// LEVEL 3 - ORCA WHIRLPOOL
// =============================================================================

pub const LEVEL_3_WHIRLPOOL: Pubkey =
    pubkey!("GMNFmkhU8hnCwofqh9gGwW8H6SqohrP8PmoJQAMycNwZ");

pub const LEVEL_3_TOKEN_A: Pubkey =
    pubkey!("4qbX8Mtx8XNt6DeCL414z67Dj9DJircMoSNEuX18AMB2");

pub const LEVEL_3_TOKEN_B: Pubkey =
    pubkey!("9b7gAMUxGdRwkEk32KtayLXAhwqib3yaTzLdvtMfvXbp");

// =============================================================================
// LEVEL 4 - ORCA WHIRLPOOL
// =============================================================================

pub const LEVEL_4_WHIRLPOOL: Pubkey =
    pubkey!("2bdPMRcKrgAvQKGfP1mW9ThNjq6rnP2nRSYmWodtdFvo");

pub const LEVEL_4_TOKEN_A: Pubkey =
    pubkey!("4qbX8Mtx8XNt6DeCL414z67Dj9DJircMoSNEuX18AMB2");

pub const LEVEL_4_TOKEN_B: Pubkey =
    pubkey!("9Zs8kUpicKNZNosFwMawxnVqFZxBfZz8dh2zLu2wahnu");

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
// TIMELOCK CONSTANTS (in seconds)
// =============================================================================

pub const DEFAULT_LEVEL_2_LOCK_DURATION: i64 = 5_184_000;  // 60 days
pub const DEFAULT_LEVEL_3_LOCK_DURATION: i64 = 7_776_000;  // 90 days
pub const DEFAULT_LEVEL_4_LOCK_DURATION: i64 = 10_368_000; // 120 days

// =============================================================================
// HELPER FUNCTIONS
// =============================================================================

pub fn get_whirlpool_for_level(level: u8) -> Option<Pubkey> {
    match level {
        1 => Some(LEVEL_1_WHIRLPOOL),
        2 => Some(LEVEL_2_WHIRLPOOL),
        3 => Some(LEVEL_3_WHIRLPOOL),
        4 => Some(LEVEL_4_WHIRLPOOL),
        _ => None,
    }
}

pub fn get_token_a_for_level(level: u8) -> Option<Pubkey> {
    match level {
        1 => Some(LEVEL_1_TOKEN_A),
        2 => Some(LEVEL_2_TOKEN_A),
        3 => Some(LEVEL_3_TOKEN_A),
        4 => Some(LEVEL_4_TOKEN_A),
        _ => None,
    }
}

pub fn get_token_b_for_level(level: u8) -> Option<Pubkey> {
    match level {
        1 => Some(LEVEL_1_TOKEN_B),
        2 => Some(LEVEL_2_TOKEN_B),
        3 => Some(LEVEL_3_TOKEN_B),
        4 => Some(LEVEL_4_TOKEN_B),
        _ => None,
    }
}

pub fn get_default_lock_duration_for_level(level: u8) -> Option<i64> {
    match level {
        2 => Some(DEFAULT_LEVEL_2_LOCK_DURATION),
        3 => Some(DEFAULT_LEVEL_3_LOCK_DURATION),
        4 => Some(DEFAULT_LEVEL_4_LOCK_DURATION),
        _ => None,
    }
}

pub fn validate_whirlpool_for_level(whirlpool: &Pubkey, level: u8) -> bool {
    match get_whirlpool_for_level(level) {
        Some(expected) => whirlpool == &expected,
        None => false,
    }
}
