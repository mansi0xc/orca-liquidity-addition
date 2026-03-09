//! Ed25519 Signature Verification for Evolution

use anchor_lang::prelude::*;
use anchor_lang::solana_program::sysvar::instructions::{
    load_current_index_checked, load_instruction_at_checked,
};
use solana_pubkey::pubkey;

use crate::constants::{EVOLUTION_SIGNATURE_DOMAIN, EVOLUTION_CANONICAL_MESSAGE_LEN};
use crate::errors::EvolutionError;

pub const ED25519_PROGRAM_ID: Pubkey = pubkey!("Ed25519SigVerify111111111111111111111111111");

pub fn verify_ed25519_instruction(
    instructions_sysvar: &AccountInfo,
    oracle_authority: &Pubkey,
    expected_message: &[u8],
) -> Result<()> {
    let current_index = load_current_index_checked(instructions_sysvar)
        .map_err(|_| EvolutionError::Ed25519InstructionNotFound)?;
    
    for idx in 0..current_index {
        let ix = load_instruction_at_checked(idx as usize, instructions_sysvar)
            .map_err(|_| EvolutionError::Ed25519InstructionNotFound)?;
        
        if ix.program_id == ED25519_PROGRAM_ID {
            return verify_ed25519_data(
                &ix.data,
                oracle_authority,
                expected_message,
            );
        }
    }
    
    Err(EvolutionError::Ed25519InstructionNotFound.into())
}

fn verify_ed25519_data(
    data: &[u8],
    oracle_authority: &Pubkey,
    expected_message: &[u8],
) -> Result<()> {
    require!(data.len() >= 16, EvolutionError::InvalidOracleSignature);
    
    let num_signatures = u16::from_le_bytes([data[0], data[1]]);
    require!(num_signatures == 1, EvolutionError::InvalidOracleSignature);
    
    let pk_offset = u16::from_le_bytes([data[6], data[7]]) as usize;
    let msg_offset = u16::from_le_bytes([data[10], data[11]]) as usize;
    let msg_size = u16::from_le_bytes([data[12], data[13]]) as usize;
    
    require!(data.len() >= pk_offset + 32, EvolutionError::InvalidOracleSignature);
    let pubkey_bytes: [u8; 32] = data[pk_offset..pk_offset + 32]
        .try_into()
        .map_err(|_| EvolutionError::InvalidOracleSignature)?;
    let instruction_pubkey = Pubkey::new_from_array(pubkey_bytes);
    require!(instruction_pubkey == *oracle_authority, EvolutionError::InvalidOracleAuthority);
    
    require!(data.len() >= msg_offset + msg_size, EvolutionError::InvalidOracleSignature);
    let instruction_msg = &data[msg_offset..msg_offset + msg_size];
    require!(instruction_msg == expected_message, EvolutionError::MessageReconstructionFailed);
    
    msg!("Evolution signature verification: PASSED");
    Ok(())
}

/// Evolution canonical message parameters.
#[derive(Debug, Clone)]
pub struct EvolutionCanonicalMessageParams {
    pub source_bond_mint: Pubkey,
    pub target_level: u8,
    pub amount_a: u64,
    pub amount_b: u64,
    pub liquidity: u128,
    pub nonce: u64,
    pub sender: Pubkey,
    pub contract_address: Pubkey,
}

/// Reconstruct evolution canonical message (155 bytes).
pub fn reconstruct_evolution_message(
    params: &EvolutionCanonicalMessageParams,
) -> [u8; EVOLUTION_CANONICAL_MESSAGE_LEN] {
    let mut message = [0u8; EVOLUTION_CANONICAL_MESSAGE_LEN];
    let mut offset = 0;
    
    // Domain (18 bytes)
    message[offset..offset + EVOLUTION_SIGNATURE_DOMAIN.len()]
        .copy_from_slice(EVOLUTION_SIGNATURE_DOMAIN);
    offset += EVOLUTION_SIGNATURE_DOMAIN.len();
    
    // source_bond_mint (32 bytes)
    message[offset..offset + 32].copy_from_slice(&params.source_bond_mint.to_bytes());
    offset += 32;
    
    // target_level (1 byte)
    message[offset] = params.target_level;
    offset += 1;
    
    // amount_a (8 bytes)
    message[offset..offset + 8].copy_from_slice(&params.amount_a.to_le_bytes());
    offset += 8;
    
    // amount_b (8 bytes)
    message[offset..offset + 8].copy_from_slice(&params.amount_b.to_le_bytes());
    offset += 8;
    
    // liquidity (16 bytes)
    message[offset..offset + 16].copy_from_slice(&params.liquidity.to_le_bytes());
    offset += 16;
    
    // nonce (8 bytes)
    message[offset..offset + 8].copy_from_slice(&params.nonce.to_le_bytes());
    offset += 8;
    
    // sender (32 bytes)
    message[offset..offset + 32].copy_from_slice(&params.sender.to_bytes());
    offset += 32;
    
    // contract_address (32 bytes)
    message[offset..offset + 32].copy_from_slice(&params.contract_address.to_bytes());
    
    message
}

pub fn verify_evolution_signature(
    instructions_sysvar: &AccountInfo,
    oracle_authority: &Pubkey,
    params: &EvolutionCanonicalMessageParams,
) -> Result<()> {
    let message = reconstruct_evolution_message(params);
    verify_ed25519_instruction(instructions_sysvar, oracle_authority, &message)
}
