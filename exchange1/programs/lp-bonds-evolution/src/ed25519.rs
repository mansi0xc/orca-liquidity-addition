//! Ed25519 Oracle Attestation Verification Module for Evolution
//!
//! Mirrors the locker's ed25519.rs — identical security model:
//! 1. Strict instruction ordering (Ed25519 at index `current - 1`)
//! 2. Extracts public key and message from Ed25519 instruction data
//! 3. Verifies pubkey matches configured oracle authority
//! 4. Verifies message matches locally reconstructed canonical message
//! 5. No signature byte comparison (runtime precompile already verified)

use anchor_lang::prelude::*;
use anchor_lang::solana_program::sysvar::instructions::{
    load_current_index_checked, load_instruction_at_checked,
};
use solana_pubkey::pubkey;

use crate::constants::EVOLUTION_CANONICAL_MESSAGE_LEN;
use crate::errors::EvolutionError;

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

/// Evolution canonical message parameters.
///
/// Includes all security-critical fields matching the locker's model:
/// whirlpool, token mints, ticks, timestamp, plus evolution-specific
/// source_bond_mint and target_level.
#[derive(Debug, Clone)]
pub struct EvolutionCanonicalMessageParams {
    /// Source bond mint being evolved.
    pub source_bond_mint: Pubkey,
    /// Target level (2, 3, or 4).
    pub target_level: u8,
    /// The Orca Whirlpool pool address for the target level.
    pub whirlpool: Pubkey,
    /// Token mint A of the pool pair.
    pub token_mint_a: Pubkey,
    /// Token mint B of the pool pair (layer token).
    pub token_mint_b: Pubkey,
    /// Token A amount transferred from user.
    pub amount_a: u64,
    /// Token B (layer token) amount minted.
    pub amount_b: u64,
    /// Liquidity amount for the position.
    pub liquidity: u128,
    /// Lower tick index of the position range (from config).
    pub tick_lower: i32,
    /// Upper tick index of the position range (from config).
    pub tick_upper: i32,
    /// Current tick of the pool at oracle signing time.
    pub tick_current: i32,
    /// Strictly sequential nonce (must == current_nonce + 1).
    pub nonce: u64,
    /// Unix timestamp when the oracle signed this message.
    pub timestamp: i64,
    /// The user (signer) initiating the evolution.
    pub sender: Pubkey,
    /// This program's ID — binds the signature to this deployment.
    pub contract_address: Pubkey,
}

/// Reconstructs the deterministic canonical message from evolution parameters.
///
/// Layout (EVOLUTION_CANONICAL_MESSAGE_LEN = 271 bytes):
/// ```text
///   [0..18)     domain separator
///   [18..50)    source_bond_mint pubkey
///   [50..51)    target_level (u8)
///   [51..83)    whirlpool pubkey
///   [83..115)   token_mint_a pubkey
///   [115..147)  token_mint_b pubkey
///   [147..155)  amount_a (u64 LE)
///   [155..163)  amount_b (u64 LE)
///   [163..179)  liquidity (u128 LE)
///   [179..183)  tick_lower (i32 LE)
///   [183..187)  tick_upper (i32 LE)
///   [187..191)  tick_current (i32 LE)
///   [191..199)  nonce (u64 LE)
///   [199..207)  timestamp (i64 LE)
///   [207..239)  sender pubkey
///   [239..271)  contract_address pubkey
/// ```
pub fn reconstruct_evolution_message(
    params: &EvolutionCanonicalMessageParams,
) -> [u8; EVOLUTION_CANONICAL_MESSAGE_LEN] {
    use crate::constants::EVOLUTION_SIGNATURE_DOMAIN;

    let mut msg = [0u8; EVOLUTION_CANONICAL_MESSAGE_LEN];
    let mut o = 0;

    msg[o..o + 18].copy_from_slice(&EVOLUTION_SIGNATURE_DOMAIN[..18]);
    o += 18;

    msg[o..o + 32].copy_from_slice(&params.source_bond_mint.to_bytes());
    o += 32;

    msg[o] = params.target_level;
    o += 1;

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

/// Verifies oracle attestation by checking the Ed25519 instruction at
/// exactly `(current_index - 1)`.
///
/// ## Security guarantees (identical to locker)
///
/// - **Strict ordering**: Ed25519 must be immediately before this instruction.
/// - **Pubkey binding**: Public key must match `oracle_authority`.
/// - **Message integrity**: Message must match `expected_message` byte-for-byte.
/// - **Exact message length**: Must be exactly EVOLUTION_CANONICAL_MESSAGE_LEN.
/// - **No signature comparison**: Precompile already verified.
/// - **No overlapping regions**: Sig, pubkey, message must be disjoint.
/// - **No trailing garbage**: Data must end at the last region boundary.
pub fn verify_evolution_signature(
    instructions_sysvar: &AccountInfo,
    oracle_authority: &Pubkey,
    params: &EvolutionCanonicalMessageParams,
) -> Result<()> {
    let expected_message = reconstruct_evolution_message(params);
    verify_oracle_attestation(instructions_sysvar, oracle_authority, &expected_message)
}

// =============================================================================
// INTERNAL HELPERS
// =============================================================================

fn verify_oracle_attestation(
    instructions_sysvar: &AccountInfo,
    oracle_authority: &Pubkey,
    expected_message: &[u8; EVOLUTION_CANONICAL_MESSAGE_LEN],
) -> Result<()> {
    let current_index = load_current_index_checked(instructions_sysvar)
        .map_err(|_| EvolutionError::InvalidEd25519Index)?;

    // STRICT: Ed25519 must be at exactly (current_index - 1).
    // Why: scanning allows an attacker to place the Ed25519 instruction
    // anywhere in the transaction, potentially reusing a stale instruction.
    require!(current_index >= 1, EvolutionError::InvalidInstructionOrder);
    let ed25519_index = (current_index - 1) as usize;

    let ix = load_instruction_at_checked(ed25519_index, instructions_sysvar)
        .map_err(|_| EvolutionError::Ed25519InstructionNotFound)?;

    require!(
        ix.program_id == ED25519_PROGRAM_ID,
        EvolutionError::InvalidInstructionOrder
    );

    extract_and_verify(&ix.data, oracle_authority, expected_message)
}

/// Parses Ed25519 instruction data and verifies pubkey + message match.
fn extract_and_verify(
    data: &[u8],
    oracle_authority: &Pubkey,
    expected_message: &[u8; EVOLUTION_CANONICAL_MESSAGE_LEN],
) -> Result<()> {
    // Minimum: 2 (header) + 14 (offsets) = 16 bytes
    require!(data.len() >= 16, EvolutionError::InvalidOracleSignature);

    let num_signatures = u16::from_le_bytes([data[0], data[1]]);
    require!(num_signatures == 1, EvolutionError::InvalidOracleSignature);

    let offsets = parse_ed25519_offsets(&data[2..])?;

    // All data must be within THIS instruction (index 0xFFFF = "this instruction").
    // Reject if any offset references a different instruction.
    require!(
        offsets.signature_instruction_index == 0xFFFF,
        EvolutionError::InvalidOracleSignature
    );
    require!(
        offsets.public_key_instruction_index == 0xFFFF,
        EvolutionError::InvalidOracleSignature
    );
    require!(
        offsets.message_instruction_index == 0xFFFF,
        EvolutionError::InvalidOracleSignature
    );

    // All regions must start at or after the 16-byte header
    require!(offsets.signature_offset >= 16, EvolutionError::InvalidOracleSignature);
    require!(offsets.public_key_offset >= 16, EvolutionError::InvalidOracleSignature);
    require!(offsets.message_offset >= 16, EvolutionError::InvalidOracleSignature);

    // --- Validate signature region bounds ---
    let sig_start = offsets.signature_offset as usize;
    let sig_end = sig_start.checked_add(64)
        .ok_or(error!(EvolutionError::InvalidOracleSignature))?;
    require!(data.len() >= sig_end, EvolutionError::InvalidOracleSignature);

    // --- Extract and verify PUBLIC KEY ---
    let pk_start = offsets.public_key_offset as usize;
    let pk_end = pk_start.checked_add(32)
        .ok_or(error!(EvolutionError::InvalidOracleSignature))?;
    require!(data.len() >= pk_end, EvolutionError::InvalidOracleSignature);

    let pubkey_bytes: [u8; 32] = data[pk_start..pk_end]
        .try_into()
        .map_err(|_| error!(EvolutionError::InvalidOracleSignature))?;
    require!(
        Pubkey::new_from_array(pubkey_bytes) == *oracle_authority,
        EvolutionError::InvalidOracleAuthority
    );

    // --- Extract and verify MESSAGE ---
    let msg_len = offsets.message_data_size as usize;
    require!(msg_len == EVOLUTION_CANONICAL_MESSAGE_LEN, EvolutionError::InvalidMessageLength);

    let msg_start = offsets.message_offset as usize;
    let msg_end = msg_start.checked_add(msg_len)
        .ok_or(error!(EvolutionError::InvalidOracleSignature))?;
    require!(data.len() >= msg_end, EvolutionError::InvalidOracleSignature);

    require!(
        &data[msg_start..msg_end] == expected_message.as_slice(),
        EvolutionError::MessageReconstructionFailed
    );

    // --- Verify no region overlaps ---
    let no_sig_pk_overlap = sig_end <= pk_start || pk_end <= sig_start;
    let no_sig_msg_overlap = sig_end <= msg_start || msg_end <= sig_start;
    let no_pk_msg_overlap = pk_end <= msg_start || msg_end <= pk_start;
    require!(no_sig_pk_overlap, EvolutionError::InvalidOracleSignature);
    require!(no_sig_msg_overlap, EvolutionError::InvalidOracleSignature);
    require!(no_pk_msg_overlap, EvolutionError::InvalidOracleSignature);

    // --- Verify no trailing garbage ---
    let max_region_end = sig_end.max(pk_end).max(msg_end);
    require!(data.len() == max_region_end, EvolutionError::InvalidOracleSignature);

    Ok(())
}

/// Parses the 14-byte Ed25519 signature offsets structure.
fn parse_ed25519_offsets(data: &[u8]) -> Result<Ed25519SignatureOffsets> {
    require!(data.len() >= 14, EvolutionError::InvalidOracleSignature);

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
    use crate::constants::EVOLUTION_SIGNATURE_DOMAIN;

    // Standard Ed25519 instruction layout:
    //   [0..2)    num_signatures (1)
    //   [2..4)    signature_offset (16)
    //   [4..6)    signature_instruction_index (0xFFFF)
    //   [6..8)    public_key_offset (80)
    //   [8..10)   public_key_instruction_index (0xFFFF)
    //   [10..12)  message_offset (112)
    //   [12..14)  message_data_size (271)
    //   [14..16)  message_instruction_index (0xFFFF)
    //   [16..80)  signature (64 bytes)
    //   [80..112) public_key (32 bytes)
    //   [112..383) message (271 bytes)
    //   Total: 383 bytes

    const SIG_OFFSET: u16 = 16;
    const PK_OFFSET: u16 = 80;   // 16 + 64
    const MSG_OFFSET: u16 = 112;  // 16 + 64 + 32
    const TOTAL_LEN: usize = 383; // 16 + 64 + 32 + 271

    fn default_params() -> EvolutionCanonicalMessageParams {
        EvolutionCanonicalMessageParams {
            source_bond_mint: Pubkey::default(),
            target_level: 2,
            whirlpool: Pubkey::default(),
            token_mint_a: Pubkey::default(),
            token_mint_b: Pubkey::default(),
            amount_a: 1_000_000,
            amount_b: 2_000_000,
            liquidity: 36583284382,
            tick_lower: -10000,
            tick_upper: 10000,
            tick_current: 500,
            nonce: 1,
            timestamp: 1700000000,
            sender: Pubkey::default(),
            contract_address: Pubkey::default(),
        }
    }

    /// Builds a valid Ed25519 instruction data blob with standard layout.
    fn build_valid_ed25519_data(
        oracle_pubkey: &Pubkey,
        message: &[u8; EVOLUTION_CANONICAL_MESSAGE_LEN],
    ) -> Vec<u8> {
        let mut data = vec![0u8; TOTAL_LEN];

        // Header
        data[0..2].copy_from_slice(&1u16.to_le_bytes()); // num_signatures = 1

        // Offsets
        data[2..4].copy_from_slice(&SIG_OFFSET.to_le_bytes());
        data[4..6].copy_from_slice(&0xFFFFu16.to_le_bytes()); // sig ix index
        data[6..8].copy_from_slice(&PK_OFFSET.to_le_bytes());
        data[8..10].copy_from_slice(&0xFFFFu16.to_le_bytes()); // pk ix index
        data[10..12].copy_from_slice(&MSG_OFFSET.to_le_bytes());
        data[12..14].copy_from_slice(
            &(EVOLUTION_CANONICAL_MESSAGE_LEN as u16).to_le_bytes(),
        );
        data[14..16].copy_from_slice(&0xFFFFu16.to_le_bytes()); // msg ix index

        // Signature (64 bytes of dummy data — precompile already verified)
        for i in 0..64 {
            data[SIG_OFFSET as usize + i] = 0xAA;
        }

        // Public key
        data[PK_OFFSET as usize..PK_OFFSET as usize + 32]
            .copy_from_slice(&oracle_pubkey.to_bytes());

        // Message
        data[MSG_OFFSET as usize..MSG_OFFSET as usize + EVOLUTION_CANONICAL_MESSAGE_LEN]
            .copy_from_slice(message);

        data
    }

    // =========================================================================
    // MESSAGE RECONSTRUCTION TESTS
    // =========================================================================

    #[test]
    fn test_message_length_is_271() {
        let msg = reconstruct_evolution_message(&default_params());
        assert_eq!(msg.len(), EVOLUTION_CANONICAL_MESSAGE_LEN);
        assert_eq!(msg.len(), 271);
    }

    #[test]
    fn test_message_is_deterministic() {
        let params = default_params();
        assert_eq!(
            reconstruct_evolution_message(&params),
            reconstruct_evolution_message(&params),
        );
    }

    #[test]
    fn test_message_domain_prefix() {
        let msg = reconstruct_evolution_message(&default_params());
        assert_eq!(&msg[0..18], EVOLUTION_SIGNATURE_DOMAIN);
        assert_eq!(&msg[0..18], b"LP_BONDS_EVOLVE_V1");
    }

    #[test]
    fn test_message_source_bond_mint_offset() {
        let mut params = default_params();
        params.source_bond_mint = Pubkey::new_unique();
        let msg = reconstruct_evolution_message(&params);
        assert_eq!(&msg[18..50], &params.source_bond_mint.to_bytes());
    }

    #[test]
    fn test_message_target_level_offset() {
        let mut params = default_params();
        params.target_level = 3;
        let msg = reconstruct_evolution_message(&params);
        assert_eq!(msg[50], 3);
    }

    #[test]
    fn test_message_nonce_offset() {
        let mut params = default_params();
        params.nonce = 0x0102030405060708;
        let msg = reconstruct_evolution_message(&params);
        assert_eq!(&msg[191..199], &0x0102030405060708u64.to_le_bytes());
    }

    #[test]
    fn test_message_timestamp_offset() {
        let mut params = default_params();
        params.timestamp = 1700000042;
        let msg = reconstruct_evolution_message(&params);
        assert_eq!(&msg[199..207], &1700000042i64.to_le_bytes());
    }

    #[test]
    fn test_message_contract_address_offset() {
        let mut params = default_params();
        params.contract_address = Pubkey::new_unique();
        let msg = reconstruct_evolution_message(&params);
        assert_eq!(&msg[239..271], &params.contract_address.to_bytes());
    }

    #[test]
    fn test_message_sender_offset() {
        let mut params = default_params();
        params.sender = Pubkey::new_unique();
        let msg = reconstruct_evolution_message(&params);
        assert_eq!(&msg[207..239], &params.sender.to_bytes());
    }

    #[test]
    fn test_message_tick_offsets() {
        let mut params = default_params();
        params.tick_lower = -443636;
        params.tick_upper = 443636;
        params.tick_current = -50000;
        let msg = reconstruct_evolution_message(&params);
        assert_eq!(&msg[179..183], &(-443636i32).to_le_bytes());
        assert_eq!(&msg[183..187], &443636i32.to_le_bytes());
        assert_eq!(&msg[187..191], &(-50000i32).to_le_bytes());
    }

    #[test]
    fn test_message_liquidity_offset() {
        let mut params = default_params();
        params.liquidity = u128::MAX;
        let msg = reconstruct_evolution_message(&params);
        assert_eq!(&msg[163..179], &u128::MAX.to_le_bytes());
    }

    #[test]
    fn test_different_nonces_produce_different_messages() {
        let mut p1 = default_params();
        p1.nonce = 1;
        let mut p2 = default_params();
        p2.nonce = 2;
        assert_ne!(
            reconstruct_evolution_message(&p1),
            reconstruct_evolution_message(&p2),
        );
    }

    #[test]
    fn test_different_senders_produce_different_messages() {
        let mut p1 = default_params();
        p1.sender = Pubkey::new_unique();
        let mut p2 = default_params();
        p2.sender = Pubkey::new_unique();
        assert_ne!(
            reconstruct_evolution_message(&p1),
            reconstruct_evolution_message(&p2),
        );
    }

    #[test]
    fn test_different_source_bond_mints_produce_different_messages() {
        let mut p1 = default_params();
        p1.source_bond_mint = Pubkey::new_unique();
        let mut p2 = default_params();
        p2.source_bond_mint = Pubkey::new_unique();
        assert_ne!(
            reconstruct_evolution_message(&p1),
            reconstruct_evolution_message(&p2),
        );
    }

    #[test]
    fn test_different_target_levels_produce_different_messages() {
        let mut p1 = default_params();
        p1.target_level = 2;
        let mut p2 = default_params();
        p2.target_level = 3;
        assert_ne!(
            reconstruct_evolution_message(&p1),
            reconstruct_evolution_message(&p2),
        );
    }

    #[test]
    fn test_different_contract_addresses_produce_different_messages() {
        let mut p1 = default_params();
        p1.contract_address = Pubkey::new_unique();
        let mut p2 = default_params();
        p2.contract_address = Pubkey::new_unique();
        assert_ne!(
            reconstruct_evolution_message(&p1),
            reconstruct_evolution_message(&p2),
        );
    }

    #[test]
    fn test_different_whirlpools_produce_different_messages() {
        let mut p1 = default_params();
        p1.whirlpool = Pubkey::new_unique();
        let mut p2 = default_params();
        p2.whirlpool = Pubkey::new_unique();
        assert_ne!(
            reconstruct_evolution_message(&p1),
            reconstruct_evolution_message(&p2),
        );
    }

    #[test]
    fn test_different_token_mints_produce_different_messages() {
        let mut p1 = default_params();
        p1.token_mint_a = Pubkey::new_unique();
        let mut p2 = default_params();
        p2.token_mint_a = Pubkey::new_unique();
        assert_ne!(
            reconstruct_evolution_message(&p1),
            reconstruct_evolution_message(&p2),
        );

        let mut p3 = default_params();
        p3.token_mint_b = Pubkey::new_unique();
        let mut p4 = default_params();
        p4.token_mint_b = Pubkey::new_unique();
        assert_ne!(
            reconstruct_evolution_message(&p3),
            reconstruct_evolution_message(&p4),
        );
    }

    #[test]
    fn test_different_amounts_produce_different_messages() {
        let mut p1 = default_params();
        p1.amount_a = 1;
        let mut p2 = default_params();
        p2.amount_a = 2;
        assert_ne!(
            reconstruct_evolution_message(&p1),
            reconstruct_evolution_message(&p2),
        );
    }

    // =========================================================================
    // EXTRACT AND VERIFY — VALID DATA
    // =========================================================================

    #[test]
    fn test_valid_data_passes() {
        let oracle = Pubkey::new_unique();
        let msg = reconstruct_evolution_message(&default_params());
        let data = build_valid_ed25519_data(&oracle, &msg);
        assert!(extract_and_verify(&data, &oracle, &msg).is_ok());
    }

    // =========================================================================
    // EXTRACT AND VERIFY — DATA TOO SHORT
    // =========================================================================

    #[test]
    fn test_empty_data_rejected() {
        let oracle = Pubkey::new_unique();
        let msg = reconstruct_evolution_message(&default_params());
        assert!(extract_and_verify(&[], &oracle, &msg).is_err());
    }

    #[test]
    fn test_data_too_short_for_header() {
        let oracle = Pubkey::new_unique();
        let msg = reconstruct_evolution_message(&default_params());
        assert!(extract_and_verify(&[0u8; 15], &oracle, &msg).is_err());
    }

    #[test]
    fn test_data_too_short_for_signature_region() {
        let oracle = Pubkey::new_unique();
        let msg = reconstruct_evolution_message(&default_params());
        let mut data = build_valid_ed25519_data(&oracle, &msg);
        data.truncate(SIG_OFFSET as usize + 63); // 1 byte short of 64-byte sig
        assert!(extract_and_verify(&data, &oracle, &msg).is_err());
    }

    #[test]
    fn test_data_too_short_for_pubkey_region() {
        let oracle = Pubkey::new_unique();
        let msg = reconstruct_evolution_message(&default_params());
        let mut data = build_valid_ed25519_data(&oracle, &msg);
        data.truncate(PK_OFFSET as usize + 31); // 1 byte short of 32-byte pk
        assert!(extract_and_verify(&data, &oracle, &msg).is_err());
    }

    #[test]
    fn test_data_too_short_for_message_region() {
        let oracle = Pubkey::new_unique();
        let msg = reconstruct_evolution_message(&default_params());
        let mut data = build_valid_ed25519_data(&oracle, &msg);
        data.truncate(MSG_OFFSET as usize + EVOLUTION_CANONICAL_MESSAGE_LEN - 1);
        assert!(extract_and_verify(&data, &oracle, &msg).is_err());
    }

    // =========================================================================
    // EXTRACT AND VERIFY — NUM SIGNATURES
    // =========================================================================

    #[test]
    fn test_zero_signatures_rejected() {
        let oracle = Pubkey::new_unique();
        let msg = reconstruct_evolution_message(&default_params());
        let mut data = build_valid_ed25519_data(&oracle, &msg);
        data[0..2].copy_from_slice(&0u16.to_le_bytes());
        assert!(extract_and_verify(&data, &oracle, &msg).is_err());
    }

    #[test]
    fn test_two_signatures_rejected() {
        let oracle = Pubkey::new_unique();
        let msg = reconstruct_evolution_message(&default_params());
        let mut data = build_valid_ed25519_data(&oracle, &msg);
        data[0..2].copy_from_slice(&2u16.to_le_bytes());
        assert!(extract_and_verify(&data, &oracle, &msg).is_err());
    }

    #[test]
    fn test_max_u16_signatures_rejected() {
        let oracle = Pubkey::new_unique();
        let msg = reconstruct_evolution_message(&default_params());
        let mut data = build_valid_ed25519_data(&oracle, &msg);
        data[0..2].copy_from_slice(&0xFFFFu16.to_le_bytes());
        assert!(extract_and_verify(&data, &oracle, &msg).is_err());
    }

    // =========================================================================
    // EXTRACT AND VERIFY — INSTRUCTION INDEX ENFORCEMENT (0xFFFF)
    // =========================================================================

    #[test]
    fn test_signature_index_not_ffff_rejected() {
        let oracle = Pubkey::new_unique();
        let msg = reconstruct_evolution_message(&default_params());
        let mut data = build_valid_ed25519_data(&oracle, &msg);
        // Set signature_instruction_index to 0 (references another instruction)
        data[4..6].copy_from_slice(&0u16.to_le_bytes());
        assert!(extract_and_verify(&data, &oracle, &msg).is_err());
    }

    #[test]
    fn test_pubkey_index_not_ffff_rejected() {
        let oracle = Pubkey::new_unique();
        let msg = reconstruct_evolution_message(&default_params());
        let mut data = build_valid_ed25519_data(&oracle, &msg);
        data[8..10].copy_from_slice(&0u16.to_le_bytes());
        assert!(extract_and_verify(&data, &oracle, &msg).is_err());
    }

    #[test]
    fn test_message_index_not_ffff_rejected() {
        let oracle = Pubkey::new_unique();
        let msg = reconstruct_evolution_message(&default_params());
        let mut data = build_valid_ed25519_data(&oracle, &msg);
        data[14..16].copy_from_slice(&0u16.to_le_bytes());
        assert!(extract_and_verify(&data, &oracle, &msg).is_err());
    }

    #[test]
    fn test_signature_index_1_rejected() {
        let oracle = Pubkey::new_unique();
        let msg = reconstruct_evolution_message(&default_params());
        let mut data = build_valid_ed25519_data(&oracle, &msg);
        // Reference instruction at index 1 — cross-instruction splice attempt
        data[4..6].copy_from_slice(&1u16.to_le_bytes());
        assert!(extract_and_verify(&data, &oracle, &msg).is_err());
    }

    #[test]
    fn test_pubkey_index_fffe_rejected() {
        let oracle = Pubkey::new_unique();
        let msg = reconstruct_evolution_message(&default_params());
        let mut data = build_valid_ed25519_data(&oracle, &msg);
        // 0xFFFE is close to 0xFFFF but NOT equal
        data[8..10].copy_from_slice(&0xFFFEu16.to_le_bytes());
        assert!(extract_and_verify(&data, &oracle, &msg).is_err());
    }

    // =========================================================================
    // EXTRACT AND VERIFY — OFFSET < 16 (HEADER OVERLAP)
    // =========================================================================

    #[test]
    fn test_signature_offset_in_header_rejected() {
        let oracle = Pubkey::new_unique();
        let msg = reconstruct_evolution_message(&default_params());
        let mut data = build_valid_ed25519_data(&oracle, &msg);
        // Point signature offset into the header area
        data[2..4].copy_from_slice(&0u16.to_le_bytes());
        assert!(extract_and_verify(&data, &oracle, &msg).is_err());
    }

    #[test]
    fn test_pubkey_offset_in_header_rejected() {
        let oracle = Pubkey::new_unique();
        let msg = reconstruct_evolution_message(&default_params());
        let mut data = build_valid_ed25519_data(&oracle, &msg);
        data[6..8].copy_from_slice(&15u16.to_le_bytes());
        assert!(extract_and_verify(&data, &oracle, &msg).is_err());
    }

    #[test]
    fn test_message_offset_in_header_rejected() {
        let oracle = Pubkey::new_unique();
        let msg = reconstruct_evolution_message(&default_params());
        let mut data = build_valid_ed25519_data(&oracle, &msg);
        data[10..12].copy_from_slice(&2u16.to_le_bytes());
        assert!(extract_and_verify(&data, &oracle, &msg).is_err());
    }

    #[test]
    fn test_offset_exactly_16_accepted() {
        // Offset 16 is exactly at the boundary — should be accepted
        let oracle = Pubkey::new_unique();
        let msg = reconstruct_evolution_message(&default_params());
        let data = build_valid_ed25519_data(&oracle, &msg);
        // Default sig offset is already 16
        assert!(extract_and_verify(&data, &oracle, &msg).is_ok());
    }

    // =========================================================================
    // EXTRACT AND VERIFY — WRONG ORACLE PUBKEY
    // =========================================================================

    #[test]
    fn test_wrong_oracle_pubkey_rejected() {
        let real_oracle = Pubkey::new_unique();
        let wrong_oracle = Pubkey::new_unique();
        let msg = reconstruct_evolution_message(&default_params());
        let data = build_valid_ed25519_data(&wrong_oracle, &msg);
        // Data contains wrong_oracle, but we verify against real_oracle
        assert!(extract_and_verify(&data, &real_oracle, &msg).is_err());
    }

    #[test]
    fn test_default_pubkey_rejected() {
        let real_oracle = Pubkey::new_unique();
        let msg = reconstruct_evolution_message(&default_params());
        let data = build_valid_ed25519_data(&Pubkey::default(), &msg);
        assert!(extract_and_verify(&data, &real_oracle, &msg).is_err());
    }

    #[test]
    fn test_correct_oracle_pubkey_accepted() {
        let oracle = Pubkey::new_unique();
        let msg = reconstruct_evolution_message(&default_params());
        let data = build_valid_ed25519_data(&oracle, &msg);
        assert!(extract_and_verify(&data, &oracle, &msg).is_ok());
    }

    // =========================================================================
    // EXTRACT AND VERIFY — MODIFIED MESSAGE PAYLOAD
    // =========================================================================

    #[test]
    fn test_single_bit_flip_in_message_rejected() {
        let oracle = Pubkey::new_unique();
        let msg = reconstruct_evolution_message(&default_params());
        let mut tampered_msg = msg;
        tampered_msg[100] ^= 1; // Flip one bit
        let data = build_valid_ed25519_data(&oracle, &tampered_msg);
        assert!(extract_and_verify(&data, &oracle, &msg).is_err());
    }

    #[test]
    fn test_modified_nonce_in_message_rejected() {
        let oracle = Pubkey::new_unique();
        let msg = reconstruct_evolution_message(&default_params());
        let mut tampered_msg = msg;
        // Nonce at offset 191..199 — modify to nonce=2
        tampered_msg[191..199].copy_from_slice(&2u64.to_le_bytes());
        let data = build_valid_ed25519_data(&oracle, &tampered_msg);
        assert!(extract_and_verify(&data, &oracle, &msg).is_err());
    }

    #[test]
    fn test_modified_sender_in_message_rejected() {
        let oracle = Pubkey::new_unique();
        let msg = reconstruct_evolution_message(&default_params());
        let mut tampered_msg = msg;
        // Sender at offset 207..239
        tampered_msg[207..239].copy_from_slice(&Pubkey::new_unique().to_bytes());
        let data = build_valid_ed25519_data(&oracle, &tampered_msg);
        assert!(extract_and_verify(&data, &oracle, &msg).is_err());
    }

    #[test]
    fn test_modified_contract_address_in_message_rejected() {
        let oracle = Pubkey::new_unique();
        let msg = reconstruct_evolution_message(&default_params());
        let mut tampered_msg = msg;
        // Contract address at offset 239..271
        tampered_msg[239..271].copy_from_slice(&Pubkey::new_unique().to_bytes());
        let data = build_valid_ed25519_data(&oracle, &tampered_msg);
        assert!(extract_and_verify(&data, &oracle, &msg).is_err());
    }

    #[test]
    fn test_modified_domain_in_message_rejected() {
        let oracle = Pubkey::new_unique();
        let msg = reconstruct_evolution_message(&default_params());
        let mut tampered_msg = msg;
        // Replace domain with locker's domain
        tampered_msg[0..18].copy_from_slice(b"LP_BONDS_MINT_V1__");
        let data = build_valid_ed25519_data(&oracle, &tampered_msg);
        assert!(extract_and_verify(&data, &oracle, &msg).is_err());
    }

    #[test]
    fn test_zeroed_message_rejected() {
        let oracle = Pubkey::new_unique();
        let msg = reconstruct_evolution_message(&default_params());
        let zeroed_msg = [0u8; EVOLUTION_CANONICAL_MESSAGE_LEN];
        let data = build_valid_ed25519_data(&oracle, &zeroed_msg);
        assert!(extract_and_verify(&data, &oracle, &msg).is_err());
    }

    // =========================================================================
    // EXTRACT AND VERIFY — WRONG MESSAGE LENGTH
    // =========================================================================

    #[test]
    fn test_message_length_too_short_rejected() {
        let oracle = Pubkey::new_unique();
        let msg = reconstruct_evolution_message(&default_params());
        let mut data = build_valid_ed25519_data(&oracle, &msg);
        // Set message_data_size to 270 (1 byte short)
        data[12..14].copy_from_slice(&270u16.to_le_bytes());
        assert!(extract_and_verify(&data, &oracle, &msg).is_err());
    }

    #[test]
    fn test_message_length_too_long_rejected() {
        let oracle = Pubkey::new_unique();
        let msg = reconstruct_evolution_message(&default_params());
        let mut data = build_valid_ed25519_data(&oracle, &msg);
        // Set message_data_size to 272 (1 byte long)
        data[12..14].copy_from_slice(&272u16.to_le_bytes());
        assert!(extract_and_verify(&data, &oracle, &msg).is_err());
    }

    #[test]
    fn test_message_length_zero_rejected() {
        let oracle = Pubkey::new_unique();
        let msg = reconstruct_evolution_message(&default_params());
        let mut data = build_valid_ed25519_data(&oracle, &msg);
        data[12..14].copy_from_slice(&0u16.to_le_bytes());
        assert!(extract_and_verify(&data, &oracle, &msg).is_err());
    }

    #[test]
    fn test_message_length_238_locker_format_rejected() {
        let oracle = Pubkey::new_unique();
        let msg = reconstruct_evolution_message(&default_params());
        let mut data = build_valid_ed25519_data(&oracle, &msg);
        // 238 is the locker's message length — must be rejected here
        data[12..14].copy_from_slice(&238u16.to_le_bytes());
        assert!(extract_and_verify(&data, &oracle, &msg).is_err());
    }

    // =========================================================================
    // EXTRACT AND VERIFY — TRAILING GARBAGE
    // =========================================================================

    #[test]
    fn test_trailing_byte_rejected() {
        let oracle = Pubkey::new_unique();
        let msg = reconstruct_evolution_message(&default_params());
        let mut data = build_valid_ed25519_data(&oracle, &msg);
        data.push(0x00); // One trailing byte
        assert!(extract_and_verify(&data, &oracle, &msg).is_err());
    }

    #[test]
    fn test_many_trailing_bytes_rejected() {
        let oracle = Pubkey::new_unique();
        let msg = reconstruct_evolution_message(&default_params());
        let mut data = build_valid_ed25519_data(&oracle, &msg);
        data.extend_from_slice(&[0xFF; 100]);
        assert!(extract_and_verify(&data, &oracle, &msg).is_err());
    }

    #[test]
    fn test_exactly_correct_length_accepted() {
        let oracle = Pubkey::new_unique();
        let msg = reconstruct_evolution_message(&default_params());
        let data = build_valid_ed25519_data(&oracle, &msg);
        assert_eq!(data.len(), TOTAL_LEN);
        assert!(extract_and_verify(&data, &oracle, &msg).is_ok());
    }

    // =========================================================================
    // EXTRACT AND VERIFY — REGION OVERLAP ATTACKS
    // =========================================================================

    #[test]
    fn test_signature_pubkey_overlap_rejected() {
        let oracle = Pubkey::new_unique();
        let msg = reconstruct_evolution_message(&default_params());
        let mut data = build_valid_ed25519_data(&oracle, &msg);
        // Move pubkey to start at offset 48 (overlaps with sig at 16..80)
        data[6..8].copy_from_slice(&48u16.to_le_bytes());
        // Need to also fix pubkey data at new location
        data[48..80].copy_from_slice(&oracle.to_bytes());
        assert!(extract_and_verify(&data, &oracle, &msg).is_err());
    }

    #[test]
    fn test_signature_message_overlap_rejected() {
        let oracle = Pubkey::new_unique();
        let msg = reconstruct_evolution_message(&default_params());
        let mut data = build_valid_ed25519_data(&oracle, &msg);
        // Move message to start at offset 70 (overlaps with sig at 16..80)
        data[10..12].copy_from_slice(&70u16.to_le_bytes());
        assert!(extract_and_verify(&data, &oracle, &msg).is_err());
    }

    #[test]
    fn test_pubkey_message_overlap_rejected() {
        let oracle = Pubkey::new_unique();
        let msg = reconstruct_evolution_message(&default_params());
        let mut data = build_valid_ed25519_data(&oracle, &msg);
        // Move message to start at offset 100 (overlaps with pk at 80..112)
        data[10..12].copy_from_slice(&100u16.to_le_bytes());
        assert!(extract_and_verify(&data, &oracle, &msg).is_err());
    }

    #[test]
    fn test_adjacent_regions_accepted() {
        // Regions that are exactly adjacent (no gap, no overlap) should pass
        let oracle = Pubkey::new_unique();
        let msg = reconstruct_evolution_message(&default_params());
        let data = build_valid_ed25519_data(&oracle, &msg);
        // Default layout: sig[16..80), pk[80..112), msg[112..383)
        // These are exactly adjacent — no overlap
        assert!(extract_and_verify(&data, &oracle, &msg).is_ok());
    }

    #[test]
    fn test_single_byte_overlap_sig_pk_rejected() {
        let oracle = Pubkey::new_unique();
        let msg = reconstruct_evolution_message(&default_params());
        let mut data = build_valid_ed25519_data(&oracle, &msg);
        // Put pubkey at offset 79 — overlaps sig by 1 byte (sig ends at 80)
        data[6..8].copy_from_slice(&79u16.to_le_bytes());
        // Also need data large enough
        let needed = 79 + 32;
        while data.len() < needed {
            data.push(0);
        }
        data[79..111].copy_from_slice(&oracle.to_bytes());
        // Will fail due to overlap OR trailing garbage
        assert!(extract_and_verify(&data, &oracle, &msg).is_err());
    }

    // =========================================================================
    // EXTRACT AND VERIFY — REORDERED REGIONS
    // =========================================================================

    #[test]
    fn test_reversed_region_order_accepted() {
        // Put regions in order: message, pubkey, signature (reversed)
        // This should work as long as no overlap and exact total length
        let oracle = Pubkey::new_unique();
        let msg = reconstruct_evolution_message(&default_params());

        let msg_off: u16 = 16;
        let pk_off: u16 = 16 + EVOLUTION_CANONICAL_MESSAGE_LEN as u16; // 287
        let sig_off: u16 = pk_off + 32; // 319
        let total = sig_off as usize + 64; // 383

        let mut data = vec![0u8; total];
        data[0..2].copy_from_slice(&1u16.to_le_bytes());
        data[2..4].copy_from_slice(&sig_off.to_le_bytes());
        data[4..6].copy_from_slice(&0xFFFFu16.to_le_bytes());
        data[6..8].copy_from_slice(&pk_off.to_le_bytes());
        data[8..10].copy_from_slice(&0xFFFFu16.to_le_bytes());
        data[10..12].copy_from_slice(&msg_off.to_le_bytes());
        data[12..14].copy_from_slice(
            &(EVOLUTION_CANONICAL_MESSAGE_LEN as u16).to_le_bytes(),
        );
        data[14..16].copy_from_slice(&0xFFFFu16.to_le_bytes());

        // Fill regions
        for i in 0..64 {
            data[sig_off as usize + i] = 0xBB;
        }
        data[pk_off as usize..pk_off as usize + 32]
            .copy_from_slice(&oracle.to_bytes());
        data[msg_off as usize..msg_off as usize + EVOLUTION_CANONICAL_MESSAGE_LEN]
            .copy_from_slice(&msg);

        assert!(extract_and_verify(&data, &oracle, &msg).is_ok());
    }

    // =========================================================================
    // EXTRACT AND VERIFY — GAPS BETWEEN REGIONS
    // =========================================================================

    #[test]
    fn test_gap_between_regions_rejected() {
        // Regions with a gap produce data.len() > max_region_end → trailing garbage rejection
        let oracle = Pubkey::new_unique();
        let msg = reconstruct_evolution_message(&default_params());
        let mut data = build_valid_ed25519_data(&oracle, &msg);
        // Add a gap: extend data by 10 bytes but don't change offsets
        data.extend_from_slice(&[0u8; 10]);
        assert!(extract_and_verify(&data, &oracle, &msg).is_err());
    }

    // =========================================================================
    // EXTRACT AND VERIFY — REPLAY RESISTANCE (nonce in message)
    // =========================================================================

    #[test]
    fn test_replay_with_same_nonce_different_message() {
        let oracle = Pubkey::new_unique();

        let mut params1 = default_params();
        params1.nonce = 1;
        let msg1 = reconstruct_evolution_message(&params1);

        // Same nonce but different amount — different message
        let mut params2 = default_params();
        params2.nonce = 1;
        params2.amount_a = 999;
        let msg2 = reconstruct_evolution_message(&params2);

        assert_ne!(msg1, msg2);

        // Data contains msg1, but we verify against msg2 → fail
        let data = build_valid_ed25519_data(&oracle, &msg1);
        assert!(extract_and_verify(&data, &oracle, &msg2).is_err());
    }

    #[test]
    fn test_sequential_nonces_produce_unique_messages() {
        let oracle = Pubkey::new_unique();
        let mut msgs = Vec::new();
        for nonce in 1..=10u64 {
            let mut params = default_params();
            params.nonce = nonce;
            msgs.push(reconstruct_evolution_message(&params));
        }
        // All messages must be unique
        for i in 0..msgs.len() {
            for j in (i + 1)..msgs.len() {
                assert_ne!(msgs[i], msgs[j], "nonce {} and {} produced same msg", i + 1, j + 1);
            }
        }

        // Each message only passes its own verification
        for (i, msg) in msgs.iter().enumerate() {
            let data = build_valid_ed25519_data(&oracle, msg);
            assert!(extract_and_verify(&data, &oracle, msg).is_ok());
            // Verify against any other message → fail
            if i + 1 < msgs.len() {
                assert!(extract_and_verify(&data, &oracle, &msgs[i + 1]).is_err());
            }
        }
    }

    // =========================================================================
    // EXTRACT AND VERIFY — CROSS-PROGRAM REPLAY PREVENTION
    // =========================================================================

    #[test]
    fn test_cross_program_replay_rejected() {
        let oracle = Pubkey::new_unique();
        let mut params = default_params();
        params.contract_address = Pubkey::new_unique();
        let msg = reconstruct_evolution_message(&params);

        // Attacker reconstructs message with different contract_address
        let mut attack_params = params.clone();
        attack_params.contract_address = Pubkey::new_unique();
        let attack_msg = reconstruct_evolution_message(&attack_params);

        // Data has real message, verify against attack message → fail
        let data = build_valid_ed25519_data(&oracle, &msg);
        assert!(extract_and_verify(&data, &oracle, &attack_msg).is_err());
    }

    // =========================================================================
    // EXTRACT AND VERIFY — DOMAIN SEPARATION
    // =========================================================================

    #[test]
    fn test_evolution_domain_is_18_bytes() {
        assert_eq!(EVOLUTION_SIGNATURE_DOMAIN.len(), 18);
    }

    #[test]
    fn test_evolution_domain_distinct_from_locker_domains() {
        assert_ne!(EVOLUTION_SIGNATURE_DOMAIN, b"LP_BONDS_MINT_V1__");
        assert_ne!(EVOLUTION_SIGNATURE_DOMAIN, b"LP_BONDS_VRFY_V1__");
    }

    // =========================================================================
    // EDGE CASES
    // =========================================================================

    #[test]
    fn test_all_zero_pubkey_in_data_vs_nonzero_oracle() {
        let oracle = Pubkey::new_unique();
        let msg = reconstruct_evolution_message(&default_params());
        let data = build_valid_ed25519_data(&Pubkey::default(), &msg);
        // Data has zero pubkey, oracle is non-zero → must reject
        assert!(extract_and_verify(&data, &oracle, &msg).is_err());
    }

    #[test]
    fn test_max_offset_values_u16() {
        // Offsets that would put regions far beyond actual data length
        let oracle = Pubkey::new_unique();
        let msg = reconstruct_evolution_message(&default_params());
        let mut data = build_valid_ed25519_data(&oracle, &msg);
        // Set signature offset to max u16
        data[2..4].copy_from_slice(&0xFFFEu16.to_le_bytes());
        // This will fail on bounds check (data.len() < sig_end)
        assert!(extract_and_verify(&data, &oracle, &msg).is_err());
    }

    #[test]
    fn test_offsets_with_checked_add_overflow() {
        // sig_start = 0xFFFE, sig_end = 0xFFFE + 64 = 65598 > u16::MAX
        // checked_add should handle this (it's usize so won't overflow,
        // but bounds check will fail since data is only 383 bytes)
        let oracle = Pubkey::new_unique();
        let msg = reconstruct_evolution_message(&default_params());
        let mut data = build_valid_ed25519_data(&oracle, &msg);
        data[2..4].copy_from_slice(&0xFFFEu16.to_le_bytes());
        assert!(extract_and_verify(&data, &oracle, &msg).is_err());
    }
}
