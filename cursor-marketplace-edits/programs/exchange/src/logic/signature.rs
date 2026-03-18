use anchor_lang::prelude::*;
use anchor_lang::solana_program::sysvar::instructions as ix_sysvar;
use anchor_lang::solana_program::ed25519_program;

use crate::errors::ExchangeError;

/// The Ed25519 instruction data layout for a single signature:
/// - Bytes 0-1: num_signatures (u16 LE)
/// - Bytes 2-3: padding
/// - Bytes 4-5: signature_offset (u16 LE)
/// - Bytes 6-7: signature_instruction_index (u16 LE)
/// - Bytes 8-9: public_key_offset (u16 LE)
/// - Bytes 10-11: public_key_instruction_index (u16 LE)
/// - Bytes 12-13: message_data_offset (u16 LE)
/// - Bytes 14-15: message_data_size (u16 LE)
/// - Bytes 16-17: message_instruction_index (u16 LE)
/// Then the actual data follows (public key, signature, message).

/// Verify an Ed25519 signature by introspecting the instructions sysvar.
/// RULE S-1 through S-4.
pub fn verify_ed25519_signature(
    instructions_sysvar: &AccountInfo,
    expected_signer: &Pubkey,
    expected_message: &[u8],
    sig_instruction_index: usize,
) -> Result<()> {
    let ix = ix_sysvar::load_instruction_at_checked(sig_instruction_index, instructions_sysvar)
        .map_err(|_| ExchangeError::InvalidEd25519Instruction)?;

    // Verify it's the Ed25519 program
    require!(
        ix.program_id == ed25519_program::ID,
        ExchangeError::InvalidEd25519Instruction
    );

    let ix_data = &ix.data;
    require!(ix_data.len() >= 2, ExchangeError::InvalidEd25519Instruction);

    let num_signatures = u16::from_le_bytes([ix_data[0], ix_data[1]]);
    require!(num_signatures >= 1, ExchangeError::InvalidEd25519Instruction);

    // Parse the first signature entry (offset 2 for padding, then 14 bytes per signature header)
    require!(ix_data.len() >= 16, ExchangeError::InvalidEd25519Instruction);

    let public_key_offset = u16::from_le_bytes([ix_data[6], ix_data[7]]) as usize;
    let message_data_offset = u16::from_le_bytes([ix_data[10], ix_data[11]]) as usize;
    let message_data_size = u16::from_le_bytes([ix_data[12], ix_data[13]]) as usize;

    // Extract and verify the public key
    require!(
        ix_data.len() >= public_key_offset + 32,
        ExchangeError::InvalidEd25519Instruction
    );
    let pubkey_bytes = &ix_data[public_key_offset..public_key_offset + 32];
    let recovered_signer = Pubkey::try_from(pubkey_bytes)
        .map_err(|_| ExchangeError::InvalidSignature)?;

    require!(
        recovered_signer == *expected_signer,
        ExchangeError::InvalidSignature
    );

    // Extract and verify the message
    require!(
        ix_data.len() >= message_data_offset + message_data_size,
        ExchangeError::InvalidEd25519Instruction
    );
    let message = &ix_data[message_data_offset..message_data_offset + message_data_size];

    require!(
        message == expected_message,
        ExchangeError::InvalidSignature
    );

    Ok(())
}
