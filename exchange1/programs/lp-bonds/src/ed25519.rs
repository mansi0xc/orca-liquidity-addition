//! Ed25519 Oracle Attestation Verification Module
//!
//! Verifies that a valid Ed25519SigVerify instruction exists at exactly
//! (current_instruction_index - 1) in the transaction, and that the signed
//! message matches the expected canonical oracle message.
//!
//! ## Security Model
//!
//! The Solana Ed25519 precompile program verifies the cryptographic signature
//! at the runtime level. If the signature is invalid, the precompile instruction
//! fails and the entire transaction is rejected — our program never executes.
//!
//! Therefore, this module does NOT re-verify the signature bytes. Instead it:
//! 1. Enforces strict instruction ordering (Ed25519 at index `current - 1`)
//! 2. Extracts the public key from the Ed25519 instruction data
//! 3. Verifies the public key matches our configured oracle authority
//! 4. Extracts the message from the Ed25519 instruction data
//! 5. Verifies the message matches our locally reconstructed canonical message
//!
//! This ensures the oracle signed exactly the parameters we expect, without
//! trusting any client-provided data.

use anchor_lang::prelude::*;
use anchor_lang::solana_program::sysvar::instructions::{
    load_current_index_checked, load_instruction_at_checked,
};
use solana_pubkey::pubkey;

use crate::constants::{ORACLE_MESSAGE_LEN, EXCHANGE_MESSAGE_LEN};
use crate::errors::LpBondsError;

/// Ed25519 signature verification precompile program ID.
pub const ED25519_PROGRAM_ID: Pubkey = pubkey!("Ed25519SigVerify111111111111111111111111111");

/// Parsed Ed25519 instruction offset structure.
/// Matches the on-chain layout: 7 × u16 = 14 bytes.
#[derive(Debug)]
struct Ed25519SignatureOffsets {
    signature_offset: u16,
    signature_instruction_index: u16,
    public_key_offset: u16,
    public_key_instruction_index: u16,
    message_offset: u16,
    message_data_size: u16,
    message_instruction_index: u16,
}

// =============================================================================
// PUBLIC API
// =============================================================================

/// Unified oracle message parameters.
///
/// Used by both `add_liquidity_and_mint_bond` and `verify_collateral`.
/// The `domain` field distinguishes the two use cases, preventing
/// cross-instruction signature replay.
#[derive(Debug, Clone)]
pub struct OracleMessageParams<'a> {
    /// 18-byte domain separator (ORACLE_DOMAIN_MINT or ORACLE_DOMAIN_VERIFY).
    pub domain: &'a [u8],
    /// The Orca Whirlpool pool address.
    pub whirlpool: Pubkey,
    /// Token mint A of the pool pair.
    pub token_mint_a: Pubkey,
    /// Token mint B of the pool pair.
    pub token_mint_b: Pubkey,
    /// Token A amount (token_max_a for mint, amount0 for verify).
    pub amount_a: u64,
    /// Token B amount (token_max_b for mint, amount1 for verify).
    pub amount_b: u64,
    /// Liquidity amount for the position.
    pub liquidity: u128,
    /// Lower tick index of the position range.
    pub tick_lower: i32,
    /// Upper tick index of the position range.
    pub tick_upper: i32,
    /// Current tick of the pool at oracle signing time.
    pub tick_current: i32,
    /// Strictly sequential nonce (must == current_nonce + 1).
    pub nonce: u64,
    /// Unix timestamp when the oracle signed this message.
    pub timestamp: i64,
    /// The user (signer) initiating the operation.
    pub sender: Pubkey,
    /// This program's ID — binds the signature to this deployment.
    pub contract_address: Pubkey,
}

/// Reconstructs the deterministic canonical message from oracle parameters.
///
/// Layout (ORACLE_MESSAGE_LEN = 238 bytes):
/// ```text
///   [0..18)     domain separator
///   [18..50)    whirlpool pubkey
///   [50..82)    token_mint_a pubkey
///   [82..114)   token_mint_b pubkey
///   [114..122)  amount_a (u64 LE)
///   [122..130)  amount_b (u64 LE)
///   [130..146)  liquidity (u128 LE)
///   [146..150)  tick_lower (i32 LE)
///   [150..154)  tick_upper (i32 LE)
///   [154..158)  tick_current (i32 LE)
///   [158..166)  nonce (u64 LE)
///   [166..174)  timestamp (i64 LE)
///   [174..206)  sender pubkey
///   [206..238)  contract_address pubkey
/// ```
pub fn reconstruct_oracle_message(params: &OracleMessageParams) -> [u8; ORACLE_MESSAGE_LEN] {
    let mut msg = [0u8; ORACLE_MESSAGE_LEN];
    let mut o = 0;

    msg[o..o + 18].copy_from_slice(&params.domain[..18]);
    o += 18;

    msg[o..o + 32].copy_from_slice(&params.whirlpool.to_bytes());
    o += 32;

    msg[o..o + 32].copy_from_slice(&params.token_mint_a.to_bytes());
    o += 32;

    msg[o..o + 32].copy_from_slice(&params.token_mint_b.to_bytes());
    o += 32;

    msg[o..o + 8].copy_from_slice(&params.amount_a.to_le_bytes());
    o += 8;

    msg[o..o + 8].copy_from_slice(&params.amount_b.to_le_bytes());
    o += 8;

    msg[o..o + 16].copy_from_slice(&params.liquidity.to_le_bytes());
    o += 16;

    msg[o..o + 4].copy_from_slice(&params.tick_lower.to_le_bytes());
    o += 4;

    msg[o..o + 4].copy_from_slice(&params.tick_upper.to_le_bytes());
    o += 4;

    msg[o..o + 4].copy_from_slice(&params.tick_current.to_le_bytes());
    o += 4;

    msg[o..o + 8].copy_from_slice(&params.nonce.to_le_bytes());
    o += 8;

    msg[o..o + 8].copy_from_slice(&params.timestamp.to_le_bytes());
    o += 8;

    msg[o..o + 32].copy_from_slice(&params.sender.to_bytes());
    o += 32;

    msg[o..o + 32].copy_from_slice(&params.contract_address.to_bytes());

    msg
}

/// Exchange oracle message parameters (EVM parity — no timestamp).
///
/// Used by `exchange_bonds`. The domain field prevents cross-instruction replay.
#[derive(Debug, Clone)]
pub struct ExchangeMessageParams {
    /// 18-byte domain separator (ORACLE_DOMAIN_EXCHANGE).
    pub domain: [u8; 18],
    /// The bond NFT mint being exchanged.
    pub bond_mint: Pubkey,
    /// Amount of output tokens to mint.
    pub amount_out: u64,
    /// PDA-per-nonce replay protection value.
    pub nonce: u64,
    /// The user (signer) initiating the exchange.
    pub sender: Pubkey,
    /// This program's ID.
    pub contract_address: Pubkey,
}

/// Reconstructs the deterministic canonical exchange message from parameters.
///
/// Layout (EXCHANGE_MESSAGE_LEN = 130 bytes, EVM parity — no timestamp):
/// ```text
///   [0..18)     domain separator
///   [18..50)    bond_mint pubkey
///   [50..58)    amount_out (u64 LE)
///   [58..66)    nonce (u64 LE)
///   [66..98)    sender pubkey
///   [98..130)   contract_address pubkey
/// ```
pub fn reconstruct_exchange_message(params: &ExchangeMessageParams) -> [u8; EXCHANGE_MESSAGE_LEN] {
    let mut msg = [0u8; EXCHANGE_MESSAGE_LEN];
    let mut o = 0;

    msg[o..o + 18].copy_from_slice(&params.domain);
    o += 18;

    msg[o..o + 32].copy_from_slice(&params.bond_mint.to_bytes());
    o += 32;

    msg[o..o + 8].copy_from_slice(&params.amount_out.to_le_bytes());
    o += 8;

    msg[o..o + 8].copy_from_slice(&params.nonce.to_le_bytes());
    o += 8;

    msg[o..o + 32].copy_from_slice(&params.sender.to_bytes());
    o += 32;

    msg[o..o + 32].copy_from_slice(&params.contract_address.to_bytes());

    msg
}

/// Verifies exchange oracle attestation using the exchange message format.
pub fn verify_exchange_attestation(
    instructions_sysvar: &AccountInfo,
    oracle_authority: &Pubkey,
    expected_message: &[u8; EXCHANGE_MESSAGE_LEN],
) -> Result<()> {
    let current_index = load_current_index_checked(instructions_sysvar)
        .map_err(|_| LpBondsError::InvalidEd25519Index)?;

    require!(current_index >= 1, LpBondsError::InvalidInstructionOrder);
    let ed25519_index = (current_index - 1) as usize;

    let ix = load_instruction_at_checked(ed25519_index, instructions_sysvar)
        .map_err(|_| LpBondsError::Ed25519InstructionNotFound)?;

    require!(
        ix.program_id == ED25519_PROGRAM_ID,
        LpBondsError::InvalidInstructionOrder
    );

    extract_and_verify_exchange(&ix.data, oracle_authority, expected_message)
}

/// Verifies oracle attestation by checking the Ed25519 instruction at
/// exactly `(current_index - 1)`.
///
/// ## Security guarantees
///
/// - **Strict ordering**: The Ed25519 instruction MUST be immediately before
///   this instruction. No scanning — a single index check.
/// - **Pubkey binding**: The public key inside the Ed25519 instruction must
///   match `oracle_authority`.
/// - **Message integrity**: The message inside the Ed25519 instruction must
///   match `expected_message` byte-for-byte.
/// - **No signature comparison**: The Ed25519 precompile already verified the
///   cryptographic signature. Re-comparing bytes adds no security.
/// - **Exact message length**: The message must be exactly ORACLE_MESSAGE_LEN.
pub fn verify_oracle_attestation(
    instructions_sysvar: &AccountInfo,
    oracle_authority: &Pubkey,
    expected_message: &[u8; ORACLE_MESSAGE_LEN],
) -> Result<()> {
    // Load our own instruction index
    let current_index = load_current_index_checked(instructions_sysvar)
        .map_err(|_| LpBondsError::InvalidEd25519Index)?;

    // STRICT: Ed25519 must be at exactly (current_index - 1).
    // current_index == 0 means no preceding instruction exists.
    require!(current_index >= 1, LpBondsError::InvalidInstructionOrder);
    let ed25519_index = (current_index - 1) as usize;

    // Load the instruction at (current_index - 1)
    let ix = load_instruction_at_checked(ed25519_index, instructions_sysvar)
        .map_err(|_| LpBondsError::Ed25519InstructionNotFound)?;

    // Must be the Ed25519 precompile program — no other program accepted
    require!(
        ix.program_id == ED25519_PROGRAM_ID,
        LpBondsError::InvalidInstructionOrder
    );

    // Parse and validate the instruction data
    extract_and_verify(&ix.data, oracle_authority, expected_message)
}

// =============================================================================
// INTERNAL HELPERS
// =============================================================================

/// Parses Ed25519 instruction data and verifies pubkey + message match.
///
/// The Ed25519 instruction data layout:
///   [0..2)   num_signatures (u16 LE) — must be 1
///   [2..16)  signature offsets struct (14 bytes)
///   [16..)   signature (64 bytes) + pubkey (32 bytes) + message (N bytes)
fn extract_and_verify(
    data: &[u8],
    oracle_authority: &Pubkey,
    expected_message: &[u8; ORACLE_MESSAGE_LEN],
) -> Result<()> {
    // Minimum: 2 (header) + 14 (offsets) = 16 bytes
    require!(data.len() >= 16, LpBondsError::InvalidOracleSignature);

    let num_signatures = u16::from_le_bytes([data[0], data[1]]);
    require!(num_signatures == 1, LpBondsError::InvalidOracleSignature);

    let offsets = parse_ed25519_offsets(&data[2..])?;

    // All data must be within THIS instruction (index 0xFFFF = "this instruction").
    // Reject if any offset references a different instruction, as that would
    // allow an attacker to splice data across instructions.
    require!(
        offsets.signature_instruction_index == 0xFFFF,
        LpBondsError::InvalidOracleSignature
    );
    require!(
        offsets.public_key_instruction_index == 0xFFFF,
        LpBondsError::InvalidOracleSignature
    );
    require!(
        offsets.message_instruction_index == 0xFFFF,
        LpBondsError::InvalidOracleSignature
    );

    // --- Validate all data region offsets ---
    // All regions must start at or after the 16-byte header (2-byte count +
    // 14-byte offsets struct) to prevent overlap with parsed header fields.
    require!(offsets.signature_offset >= 16, LpBondsError::InvalidOracleSignature);
    require!(offsets.public_key_offset >= 16, LpBondsError::InvalidOracleSignature);
    require!(offsets.message_offset >= 16, LpBondsError::InvalidOracleSignature);

    // --- Validate signature region bounds ---
    let sig_start = offsets.signature_offset as usize;
    let sig_end = sig_start.checked_add(64)
        .ok_or(error!(LpBondsError::InvalidOracleSignature))?;
    require!(data.len() >= sig_end, LpBondsError::InvalidOracleSignature);

    // --- Extract and verify PUBLIC KEY ---
    let pk_start = offsets.public_key_offset as usize;
    let pk_end = pk_start.checked_add(32)
        .ok_or(error!(LpBondsError::InvalidOracleSignature))?;
    require!(data.len() >= pk_end, LpBondsError::InvalidOracleSignature);

    let pubkey_bytes: [u8; 32] = data[pk_start..pk_end]
        .try_into()
        .map_err(|_| error!(LpBondsError::InvalidOracleSignature))?;
    require!(
        Pubkey::new_from_array(pubkey_bytes) == *oracle_authority,
        LpBondsError::InvalidOracleAuthority
    );

    // --- Extract and verify MESSAGE ---
    let msg_len = offsets.message_data_size as usize;
    // Exact length match — prevents truncation or extension attacks
    require!(msg_len == ORACLE_MESSAGE_LEN, LpBondsError::InvalidMessageLength);

    let msg_start = offsets.message_offset as usize;
    let msg_end = msg_start.checked_add(msg_len)
        .ok_or(error!(LpBondsError::InvalidOracleSignature))?;
    require!(data.len() >= msg_end, LpBondsError::InvalidOracleSignature);

    require!(
        &data[msg_start..msg_end] == expected_message.as_slice(),
        LpBondsError::MessageReconstructionFailed
    );

    // --- Verify no region overlaps ---
    // Overlapping data regions in the Ed25519 instruction could allow
    // ambiguous interpretation of bytes. Each region (signature: 64B,
    // pubkey: 32B, message: ORACLE_MESSAGE_LEN B) must be disjoint.
    // Two ranges [a_start, a_end) and [b_start, b_end) overlap iff
    // a_start < b_end AND b_start < a_end.
    let no_sig_pk_overlap = sig_end <= pk_start || pk_end <= sig_start;
    let no_sig_msg_overlap = sig_end <= msg_start || msg_end <= sig_start;
    let no_pk_msg_overlap = pk_end <= msg_start || msg_end <= pk_start;
    require!(no_sig_pk_overlap, LpBondsError::InvalidOracleSignature);
    require!(no_sig_msg_overlap, LpBondsError::InvalidOracleSignature);
    require!(no_pk_msg_overlap, LpBondsError::InvalidOracleSignature);

    // --- Verify no trailing garbage after data regions ---
    // Reject instruction data that extends beyond the last region.
    // This prevents unused trailing bytes that could mask crafted payloads.
    let max_region_end = sig_end.max(pk_end).max(msg_end);
    require!(data.len() == max_region_end, LpBondsError::InvalidOracleSignature);

    // NOTE: We intentionally do NOT compare signature bytes.
    // The Ed25519 precompile already verified the signature at the runtime
    // level. If verification had failed, the transaction would have been
    // rejected before our program ever executed. Re-comparing the raw bytes
    // provides zero additional security and wastes compute.

    Ok(())
}

/// Exchange-specific Ed25519 verification.
/// Identical to extract_and_verify but uses EXCHANGE_MESSAGE_LEN.
fn extract_and_verify_exchange(
    data: &[u8],
    oracle_authority: &Pubkey,
    expected_message: &[u8; EXCHANGE_MESSAGE_LEN],
) -> Result<()> {
    require!(data.len() >= 16, LpBondsError::InvalidOracleSignature);

    let num_signatures = u16::from_le_bytes([data[0], data[1]]);
    require!(num_signatures == 1, LpBondsError::InvalidOracleSignature);

    let offsets = parse_ed25519_offsets(&data[2..])?;

    require!(offsets.signature_instruction_index == 0xFFFF, LpBondsError::InvalidOracleSignature);
    require!(offsets.public_key_instruction_index == 0xFFFF, LpBondsError::InvalidOracleSignature);
    require!(offsets.message_instruction_index == 0xFFFF, LpBondsError::InvalidOracleSignature);

    require!(offsets.signature_offset >= 16, LpBondsError::InvalidOracleSignature);
    require!(offsets.public_key_offset >= 16, LpBondsError::InvalidOracleSignature);
    require!(offsets.message_offset >= 16, LpBondsError::InvalidOracleSignature);

    let sig_start = offsets.signature_offset as usize;
    let sig_end = sig_start.checked_add(64).ok_or(error!(LpBondsError::InvalidOracleSignature))?;
    require!(data.len() >= sig_end, LpBondsError::InvalidOracleSignature);

    let pk_start = offsets.public_key_offset as usize;
    let pk_end = pk_start.checked_add(32).ok_or(error!(LpBondsError::InvalidOracleSignature))?;
    require!(data.len() >= pk_end, LpBondsError::InvalidOracleSignature);

    let pubkey_bytes: [u8; 32] = data[pk_start..pk_end]
        .try_into()
        .map_err(|_| error!(LpBondsError::InvalidOracleSignature))?;
    require!(
        Pubkey::new_from_array(pubkey_bytes) == *oracle_authority,
        LpBondsError::InvalidOracleAuthority
    );

    let msg_len = offsets.message_data_size as usize;
    require!(msg_len == EXCHANGE_MESSAGE_LEN, LpBondsError::InvalidMessageLength);

    let msg_start = offsets.message_offset as usize;
    let msg_end = msg_start.checked_add(msg_len).ok_or(error!(LpBondsError::InvalidOracleSignature))?;
    require!(data.len() >= msg_end, LpBondsError::InvalidOracleSignature);

    require!(
        &data[msg_start..msg_end] == expected_message.as_slice(),
        LpBondsError::MessageReconstructionFailed
    );

    let no_sig_pk_overlap = sig_end <= pk_start || pk_end <= sig_start;
    let no_sig_msg_overlap = sig_end <= msg_start || msg_end <= sig_start;
    let no_pk_msg_overlap = pk_end <= msg_start || msg_end <= pk_start;
    require!(no_sig_pk_overlap, LpBondsError::InvalidOracleSignature);
    require!(no_sig_msg_overlap, LpBondsError::InvalidOracleSignature);
    require!(no_pk_msg_overlap, LpBondsError::InvalidOracleSignature);

    let max_region_end = sig_end.max(pk_end).max(msg_end);
    require!(data.len() == max_region_end, LpBondsError::InvalidOracleSignature);

    Ok(())
}

/// Parses the 14-byte Ed25519 signature offsets structure.
fn parse_ed25519_offsets(data: &[u8]) -> Result<Ed25519SignatureOffsets> {
    require!(data.len() >= 14, LpBondsError::InvalidOracleSignature);

    Ok(Ed25519SignatureOffsets {
        signature_offset: u16::from_le_bytes([data[0], data[1]]),
        signature_instruction_index: u16::from_le_bytes([data[2], data[3]]),
        public_key_offset: u16::from_le_bytes([data[4], data[5]]),
        public_key_instruction_index: u16::from_le_bytes([data[6], data[7]]),
        message_offset: u16::from_le_bytes([data[8], data[9]]),
        message_data_size: u16::from_le_bytes([data[10], data[11]]),
        message_instruction_index: u16::from_le_bytes([data[12], data[13]]),
    })
}

// =============================================================================
// UNIT TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::constants::{ORACLE_DOMAIN_MINT, ORACLE_DOMAIN_VERIFY};

    fn default_params(domain: &'static [u8]) -> OracleMessageParams<'static> {
        OracleMessageParams {
            domain,
            whirlpool: Pubkey::default(),
            token_mint_a: Pubkey::default(),
            token_mint_b: Pubkey::default(),
            amount_a: 0,
            amount_b: 0,
            liquidity: 0,
            tick_lower: 0,
            tick_upper: 0,
            tick_current: 0,
            nonce: 0,
            timestamp: 0,
            sender: Pubkey::default(),
            contract_address: Pubkey::default(),
        }
    }

    #[test]
    fn test_message_length() {
        let msg = reconstruct_oracle_message(&default_params(ORACLE_DOMAIN_MINT));
        assert_eq!(msg.len(), ORACLE_MESSAGE_LEN);
    }

    #[test]
    fn test_deterministic() {
        let params = OracleMessageParams {
            domain: ORACLE_DOMAIN_MINT,
            whirlpool: Pubkey::new_unique(),
            token_mint_a: Pubkey::new_unique(),
            token_mint_b: Pubkey::new_unique(),
            amount_a: 1_000_000,
            amount_b: 2_000_000,
            liquidity: 36583284382,
            tick_lower: -10000,
            tick_upper: 10000,
            tick_current: 500,
            nonce: 42,
            timestamp: 1700000000,
            sender: Pubkey::new_unique(),
            contract_address: Pubkey::new_unique(),
        };
        assert_eq!(
            reconstruct_oracle_message(&params),
            reconstruct_oracle_message(&params),
        );
    }

    #[test]
    fn test_different_domains_produce_different_messages() {
        let mut params_mint = default_params(ORACLE_DOMAIN_MINT);
        params_mint.nonce = 1;
        let mut params_verify = default_params(ORACLE_DOMAIN_VERIFY);
        params_verify.nonce = 1;

        let msg_mint = reconstruct_oracle_message(&params_mint);
        let msg_verify = reconstruct_oracle_message(&params_verify);
        assert_ne!(msg_mint, msg_verify);
    }

    #[test]
    fn test_domains_same_length() {
        assert_eq!(ORACLE_DOMAIN_MINT.len(), 18);
        assert_eq!(ORACLE_DOMAIN_VERIFY.len(), 18);
    }

    #[test]
    fn test_domains_are_distinct() {
        assert_ne!(ORACLE_DOMAIN_MINT, ORACLE_DOMAIN_VERIFY);
    }

    #[test]
    fn test_different_nonces_produce_different_messages() {
        let mut p1 = default_params(ORACLE_DOMAIN_MINT);
        p1.nonce = 1;
        let mut p2 = default_params(ORACLE_DOMAIN_MINT);
        p2.nonce = 2;
        assert_ne!(
            reconstruct_oracle_message(&p1),
            reconstruct_oracle_message(&p2),
        );
    }

    #[test]
    fn test_different_senders_produce_different_messages() {
        let mut p1 = default_params(ORACLE_DOMAIN_MINT);
        p1.sender = Pubkey::new_unique();
        let mut p2 = default_params(ORACLE_DOMAIN_MINT);
        p2.sender = Pubkey::new_unique();
        assert_ne!(
            reconstruct_oracle_message(&p1),
            reconstruct_oracle_message(&p2),
        );
    }

    #[test]
    fn test_different_token_mints_produce_different_messages() {
        let mut p1 = default_params(ORACLE_DOMAIN_MINT);
        p1.token_mint_a = Pubkey::new_unique();
        let mut p2 = default_params(ORACLE_DOMAIN_MINT);
        p2.token_mint_a = Pubkey::new_unique();
        assert_ne!(
            reconstruct_oracle_message(&p1),
            reconstruct_oracle_message(&p2),
        );
    }

    #[test]
    fn test_message_layout_domain_prefix() {
        let msg = reconstruct_oracle_message(&default_params(ORACLE_DOMAIN_MINT));
        assert_eq!(&msg[0..18], ORACLE_DOMAIN_MINT);
    }

    #[test]
    fn test_message_layout_nonce_offset() {
        let mut params = default_params(ORACLE_DOMAIN_MINT);
        params.nonce = 0x0102030405060708;
        let msg = reconstruct_oracle_message(&params);
        // nonce at offset 158..166
        assert_eq!(
            &msg[158..166],
            &0x0102030405060708u64.to_le_bytes(),
        );
    }

    // =========================================================================
    // MULTI-INSTRUCTION NONCE PROGRESSION & ORACLE ISOLATION TESTS
    //
    // These tests validate that the nonce + oracle system supports multiple
    // add_liquidity_and_mint_bond instructions in a single transaction at the
    // logical level. Each instruction requires:
    //   - Its own Ed25519 instruction immediately before it
    //   - A strictly sequential nonce (current + 1)
    //   - A unique oracle message (nonce makes each unique)
    //
    // Transaction layout for N mints:
    //   [Ed25519(nonce=1), Mint(nonce=1), Ed25519(nonce=2), Mint(nonce=2), ...]
    //
    // NOTE: While the logic supports this, the 1232-byte Solana transaction
    // size limit prevents fitting 2+ mints in practice. Each Ed25519 instruction
    // carries ~350 bytes of payload (16 header + 64 sig + 32 pk + 238 msg),
    // so two Ed25519 instructions alone consume 700 of 1232 bytes.
    // =========================================================================

    // -- Helper: build valid Ed25519 instruction data --
    const SIG_OFFSET: u16 = 16;
    const PK_OFFSET: u16 = 80;   // 16 + 64
    const MSG_OFFSET: u16 = 112;  // 16 + 64 + 32
    const TOTAL_LEN: usize = 16 + 64 + 32 + ORACLE_MESSAGE_LEN; // 16+64+32+238 = 350

    fn build_valid_ed25519_data(
        oracle_pubkey: &Pubkey,
        message: &[u8; ORACLE_MESSAGE_LEN],
    ) -> Vec<u8> {
        let mut data = vec![0u8; TOTAL_LEN];

        data[0..2].copy_from_slice(&1u16.to_le_bytes()); // num_signatures = 1
        data[2..4].copy_from_slice(&SIG_OFFSET.to_le_bytes());
        data[4..6].copy_from_slice(&0xFFFFu16.to_le_bytes());
        data[6..8].copy_from_slice(&PK_OFFSET.to_le_bytes());
        data[8..10].copy_from_slice(&0xFFFFu16.to_le_bytes());
        data[10..12].copy_from_slice(&MSG_OFFSET.to_le_bytes());
        data[12..14].copy_from_slice(&(ORACLE_MESSAGE_LEN as u16).to_le_bytes());
        data[14..16].copy_from_slice(&0xFFFFu16.to_le_bytes());

        for i in 0..64 {
            data[SIG_OFFSET as usize + i] = 0xAA;
        }
        data[PK_OFFSET as usize..PK_OFFSET as usize + 32]
            .copy_from_slice(&oracle_pubkey.to_bytes());
        data[MSG_OFFSET as usize..MSG_OFFSET as usize + ORACLE_MESSAGE_LEN]
            .copy_from_slice(message);

        data
    }

    #[test]
    fn test_sequential_nonces_produce_unique_messages_for_multi_mint() {
        // Simulates sequential nonce progression as would occur across
        // multiple add_liquidity_and_mint_bond in one transaction.
        let user = Pubkey::new_unique();
        let whirlpool = Pubkey::new_unique();
        let program_id = Pubkey::new_unique();
        let mint_a = Pubkey::new_unique();
        let mint_b = Pubkey::new_unique();

        let mut messages = Vec::new();
        for nonce in 1..=5u64 {
            let params = OracleMessageParams {
                domain: ORACLE_DOMAIN_MINT,
                whirlpool,
                token_mint_a: mint_a,
                token_mint_b: mint_b,
                amount_a: 1_000_000,
                amount_b: 2_000_000,
                liquidity: 36583284382,
                tick_lower: -10000,
                tick_upper: 10000,
                tick_current: 500,
                nonce,
                timestamp: 1700000000,
                sender: user,
                contract_address: program_id,
            };
            messages.push(reconstruct_oracle_message(&params));
        }

        // All 5 messages must be mutually unique
        for i in 0..messages.len() {
            for j in (i + 1)..messages.len() {
                assert_ne!(
                    messages[i], messages[j],
                    "nonce {} and {} produced identical messages",
                    i + 1,
                    j + 1
                );
            }
        }
    }

    #[test]
    fn test_oracle_isolation_nonce_n_cannot_verify_nonce_n_plus_1() {
        // Simulates what happens if instruction at index 3 tries to use
        // the Ed25519 instruction meant for index 1 (wrong nonce).
        let oracle = Pubkey::new_unique();
        let user = Pubkey::new_unique();

        let params_nonce_1 = OracleMessageParams {
            domain: ORACLE_DOMAIN_MINT,
            whirlpool: Pubkey::new_unique(),
            token_mint_a: Pubkey::new_unique(),
            token_mint_b: Pubkey::new_unique(),
            amount_a: 1_000_000,
            amount_b: 2_000_000,
            liquidity: 100_000,
            tick_lower: -10000,
            tick_upper: 10000,
            tick_current: 0,
            nonce: 1,
            timestamp: 1700000000,
            sender: user,
            contract_address: Pubkey::new_unique(),
        };
        let msg_nonce_1 = reconstruct_oracle_message(&params_nonce_1);

        let mut params_nonce_2 = params_nonce_1.clone();
        params_nonce_2.nonce = 2;
        let msg_nonce_2 = reconstruct_oracle_message(&params_nonce_2);

        // Ed25519 data contains nonce=1 message
        let data = build_valid_ed25519_data(&oracle, &msg_nonce_1);

        // Verifying against nonce=1 message → pass
        assert!(extract_and_verify(&data, &oracle, &msg_nonce_1).is_ok());

        // Verifying against nonce=2 message → fail (oracle isolation)
        assert!(extract_and_verify(&data, &oracle, &msg_nonce_2).is_err());
    }

    #[test]
    fn test_replay_same_nonce_same_params_rejected_by_nonce_logic() {
        // After nonce is committed (current_nonce = N), replaying the
        // same oracle message with nonce=N fails because the program
        // requires nonce == current_nonce + 1.
        //
        // This test validates at the message level: the nonce field in
        // the oracle message is part of the signed data. A replay would
        // need a new signature with the correct next nonce.

        let oracle = Pubkey::new_unique();

        let params = OracleMessageParams {
            domain: ORACLE_DOMAIN_MINT,
            whirlpool: Pubkey::new_unique(),
            token_mint_a: Pubkey::new_unique(),
            token_mint_b: Pubkey::new_unique(),
            amount_a: 1_000_000,
            amount_b: 2_000_000,
            liquidity: 100_000,
            tick_lower: -10000,
            tick_upper: 10000,
            tick_current: 0,
            nonce: 1,
            timestamp: 1700000000,
            sender: Pubkey::new_unique(),
            contract_address: Pubkey::new_unique(),
        };
        let msg = reconstruct_oracle_message(&params);

        // First verification succeeds
        let data = build_valid_ed25519_data(&oracle, &msg);
        assert!(extract_and_verify(&data, &oracle, &msg).is_ok());

        // After nonce committed to 1, next instruction expects nonce=2.
        // The old message (nonce=1) cannot match the expected message (nonce=2).
        let mut params_next = params.clone();
        params_next.nonce = 2;
        let expected_msg_next = reconstruct_oracle_message(&params_next);

        // Same Ed25519 data (nonce=1) fails against expected nonce=2 message
        assert!(extract_and_verify(&data, &oracle, &expected_msg_next).is_err());
    }

    #[test]
    fn test_each_extract_verify_is_independent() {
        // Validates that extract_and_verify is a pure function with no
        // hidden state — calling it multiple times with different valid
        // inputs always works, and calling with invalid inputs never
        // poisons subsequent calls.
        let oracle = Pubkey::new_unique();

        let mut params1 = default_params(ORACLE_DOMAIN_MINT);
        params1.nonce = 1;
        let msg1 = reconstruct_oracle_message(&params1);
        let data1 = build_valid_ed25519_data(&oracle, &msg1);

        let mut params2 = default_params(ORACLE_DOMAIN_MINT);
        params2.nonce = 2;
        let msg2 = reconstruct_oracle_message(&params2);
        let data2 = build_valid_ed25519_data(&oracle, &msg2);

        // Both succeed independently
        assert!(extract_and_verify(&data1, &oracle, &msg1).is_ok());
        assert!(extract_and_verify(&data2, &oracle, &msg2).is_ok());

        // Cross-verification fails
        assert!(extract_and_verify(&data1, &oracle, &msg2).is_err());
        assert!(extract_and_verify(&data2, &oracle, &msg1).is_err());

        // Original still works after cross-verification failures
        assert!(extract_and_verify(&data1, &oracle, &msg1).is_ok());
        assert!(extract_and_verify(&data2, &oracle, &msg2).is_ok());
    }

    #[test]
    fn test_position_custody_pda_uniqueness_per_bond_mint() {
        // Each bond_mint produces a unique PositionCustody PDA.
        // This guarantees no accidental account reuse across instructions.
        let program_id = Pubkey::new_unique();
        let bond_mint_1 = Pubkey::new_unique();
        let bond_mint_2 = Pubkey::new_unique();

        let (pda1, _) = Pubkey::find_program_address(
            &[b"position_custody", bond_mint_1.as_ref()],
            &program_id,
        );
        let (pda2, _) = Pubkey::find_program_address(
            &[b"position_custody", bond_mint_2.as_ref()],
            &program_id,
        );

        assert_ne!(pda1, pda2);
    }

    #[test]
    fn test_nonce_pda_is_per_user() {
        // Different users have different nonce PDAs — no cross-user interference.
        let program_id = Pubkey::new_unique();
        let user1 = Pubkey::new_unique();
        let user2 = Pubkey::new_unique();

        let (nonce_pda1, _) = Pubkey::find_program_address(
            &[b"nonce", user1.as_ref()],
            &program_id,
        );
        let (nonce_pda2, _) = Pubkey::find_program_address(
            &[b"nonce", user2.as_ref()],
            &program_id,
        );

        assert_ne!(nonce_pda1, nonce_pda2);
    }

    // =========================================================================
    // EXTRACT AND VERIFY — ADVERSARIAL TESTS
    // =========================================================================

    #[test]
    fn test_valid_data_passes() {
        let oracle = Pubkey::new_unique();
        let msg = reconstruct_oracle_message(&default_params(ORACLE_DOMAIN_MINT));
        let data = build_valid_ed25519_data(&oracle, &msg);
        assert!(extract_and_verify(&data, &oracle, &msg).is_ok());
    }

    #[test]
    fn test_empty_data_rejected() {
        let oracle = Pubkey::new_unique();
        let msg = reconstruct_oracle_message(&default_params(ORACLE_DOMAIN_MINT));
        assert!(extract_and_verify(&[], &oracle, &msg).is_err());
    }

    #[test]
    fn test_data_too_short_for_header() {
        let oracle = Pubkey::new_unique();
        let msg = reconstruct_oracle_message(&default_params(ORACLE_DOMAIN_MINT));
        assert!(extract_and_verify(&[0u8; 15], &oracle, &msg).is_err());
    }

    #[test]
    fn test_zero_signatures_rejected() {
        let oracle = Pubkey::new_unique();
        let msg = reconstruct_oracle_message(&default_params(ORACLE_DOMAIN_MINT));
        let mut data = build_valid_ed25519_data(&oracle, &msg);
        data[0..2].copy_from_slice(&0u16.to_le_bytes());
        assert!(extract_and_verify(&data, &oracle, &msg).is_err());
    }

    #[test]
    fn test_two_signatures_rejected() {
        let oracle = Pubkey::new_unique();
        let msg = reconstruct_oracle_message(&default_params(ORACLE_DOMAIN_MINT));
        let mut data = build_valid_ed25519_data(&oracle, &msg);
        data[0..2].copy_from_slice(&2u16.to_le_bytes());
        assert!(extract_and_verify(&data, &oracle, &msg).is_err());
    }

    #[test]
    fn test_signature_index_not_ffff_rejected() {
        let oracle = Pubkey::new_unique();
        let msg = reconstruct_oracle_message(&default_params(ORACLE_DOMAIN_MINT));
        let mut data = build_valid_ed25519_data(&oracle, &msg);
        data[4..6].copy_from_slice(&0u16.to_le_bytes());
        assert!(extract_and_verify(&data, &oracle, &msg).is_err());
    }

    #[test]
    fn test_pubkey_index_not_ffff_rejected() {
        let oracle = Pubkey::new_unique();
        let msg = reconstruct_oracle_message(&default_params(ORACLE_DOMAIN_MINT));
        let mut data = build_valid_ed25519_data(&oracle, &msg);
        data[8..10].copy_from_slice(&1u16.to_le_bytes());
        assert!(extract_and_verify(&data, &oracle, &msg).is_err());
    }

    #[test]
    fn test_message_index_not_ffff_rejected() {
        let oracle = Pubkey::new_unique();
        let msg = reconstruct_oracle_message(&default_params(ORACLE_DOMAIN_MINT));
        let mut data = build_valid_ed25519_data(&oracle, &msg);
        data[14..16].copy_from_slice(&0u16.to_le_bytes());
        assert!(extract_and_verify(&data, &oracle, &msg).is_err());
    }

    #[test]
    fn test_offset_in_header_rejected() {
        let oracle = Pubkey::new_unique();
        let msg = reconstruct_oracle_message(&default_params(ORACLE_DOMAIN_MINT));
        let mut data = build_valid_ed25519_data(&oracle, &msg);
        data[2..4].copy_from_slice(&0u16.to_le_bytes()); // sig offset in header
        assert!(extract_and_verify(&data, &oracle, &msg).is_err());
    }

    #[test]
    fn test_wrong_oracle_pubkey_rejected() {
        let real_oracle = Pubkey::new_unique();
        let wrong_oracle = Pubkey::new_unique();
        let msg = reconstruct_oracle_message(&default_params(ORACLE_DOMAIN_MINT));
        let data = build_valid_ed25519_data(&wrong_oracle, &msg);
        assert!(extract_and_verify(&data, &real_oracle, &msg).is_err());
    }

    #[test]
    fn test_single_bit_flip_in_message_rejected() {
        let oracle = Pubkey::new_unique();
        let msg = reconstruct_oracle_message(&default_params(ORACLE_DOMAIN_MINT));
        let mut tampered = msg;
        tampered[100] ^= 1;
        let data = build_valid_ed25519_data(&oracle, &tampered);
        assert!(extract_and_verify(&data, &oracle, &msg).is_err());
    }

    #[test]
    fn test_wrong_message_length_rejected() {
        let oracle = Pubkey::new_unique();
        let msg = reconstruct_oracle_message(&default_params(ORACLE_DOMAIN_MINT));
        let mut data = build_valid_ed25519_data(&oracle, &msg);
        // Set message_data_size to 237 (wrong)
        data[12..14].copy_from_slice(&237u16.to_le_bytes());
        assert!(extract_and_verify(&data, &oracle, &msg).is_err());
    }

    #[test]
    fn test_trailing_byte_rejected() {
        let oracle = Pubkey::new_unique();
        let msg = reconstruct_oracle_message(&default_params(ORACLE_DOMAIN_MINT));
        let mut data = build_valid_ed25519_data(&oracle, &msg);
        data.push(0x00);
        assert!(extract_and_verify(&data, &oracle, &msg).is_err());
    }

    #[test]
    fn test_signature_pubkey_overlap_rejected() {
        let oracle = Pubkey::new_unique();
        let msg = reconstruct_oracle_message(&default_params(ORACLE_DOMAIN_MINT));
        let mut data = build_valid_ed25519_data(&oracle, &msg);
        // Move pubkey to overlap with signature
        data[6..8].copy_from_slice(&48u16.to_le_bytes());
        data[48..80].copy_from_slice(&oracle.to_bytes());
        assert!(extract_and_verify(&data, &oracle, &msg).is_err());
    }

    #[test]
    fn test_pubkey_message_overlap_rejected() {
        let oracle = Pubkey::new_unique();
        let msg = reconstruct_oracle_message(&default_params(ORACLE_DOMAIN_MINT));
        let mut data = build_valid_ed25519_data(&oracle, &msg);
        // Move message to overlap with pubkey
        data[10..12].copy_from_slice(&100u16.to_le_bytes());
        assert!(extract_and_verify(&data, &oracle, &msg).is_err());
    }
}
