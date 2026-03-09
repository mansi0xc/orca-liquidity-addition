//! Ed25519 Signature Verification Helper Module
//!
//! Provides utilities for verifying Ed25519 signatures on-chain
//! using the Solana Ed25519 precompile program.

use anchor_lang::prelude::*;
use anchor_lang::solana_program::sysvar::instructions::{
    load_current_index_checked, load_instruction_at_checked,
};
use solana_pubkey::pubkey;

use crate::constants::{SIGNATURE_DOMAIN, CANONICAL_MESSAGE_LEN};
use crate::errors::LpBondsError;

pub const ED25519_PROGRAM_ID: Pubkey = pubkey!("Ed25519SigVerify111111111111111111111111111");

#[derive(Debug)]
pub struct Ed25519SignatureOffsets {
    pub signature_offset: u16,
    pub signature_instruction_index: u16,
    pub public_key_offset: u16,
    pub public_key_instruction_index: u16,
    pub message_offset: u16,
    pub message_data_size: u16,
    pub message_instruction_index: u16,
}

pub fn verify_ed25519_instruction(
    instructions_sysvar: &AccountInfo,
    oracle_authority: &Pubkey,
    expected_signature: &[u8; 64],
    expected_message: &[u8],
) -> Result<()> {
    let current_index = load_current_index_checked(instructions_sysvar)
        .map_err(|_| LpBondsError::Ed25519InstructionNotFound)?;
    
    for idx in 0..current_index {
        let ix = load_instruction_at_checked(idx as usize, instructions_sysvar)
            .map_err(|_| LpBondsError::Ed25519InstructionNotFound)?;
        
        if ix.program_id == ED25519_PROGRAM_ID {
            return verify_ed25519_instruction_data(
                &ix.data,
                oracle_authority,
                expected_signature,
                expected_message,
            );
        }
    }
    
    Err(LpBondsError::Ed25519InstructionNotFound.into())
}

fn verify_ed25519_instruction_data(
    data: &[u8],
    oracle_authority: &Pubkey,
    expected_signature: &[u8; 64],
    expected_message: &[u8],
) -> Result<()> {
    require!(data.len() >= 2 + 14, LpBondsError::InvalidOracleSignature);
    
    let num_signatures = u16::from_le_bytes([data[0], data[1]]);
    require!(num_signatures == 1, LpBondsError::InvalidOracleSignature);
    
    let offsets = parse_ed25519_offsets(&data[2..])?;
    
    require!(offsets.signature_instruction_index == 0xFFFF, LpBondsError::InvalidOracleSignature);
    require!(offsets.public_key_instruction_index == 0xFFFF, LpBondsError::InvalidOracleSignature);
    require!(offsets.message_instruction_index == 0xFFFF, LpBondsError::InvalidOracleSignature);
    
    let pk_start = offsets.public_key_offset as usize;
    let pk_end = pk_start + 32;
    require!(data.len() >= pk_end, LpBondsError::InvalidOracleSignature);
    
    let pubkey_bytes: [u8; 32] = data[pk_start..pk_end]
        .try_into()
        .map_err(|_| LpBondsError::InvalidOracleSignature)?;
    let instruction_pubkey = Pubkey::new_from_array(pubkey_bytes);
    
    require!(instruction_pubkey == *oracle_authority, LpBondsError::InvalidOracleAuthority);
    
    let sig_start = offsets.signature_offset as usize;
    let sig_end = sig_start + 64;
    require!(data.len() >= sig_end, LpBondsError::InvalidOracleSignature);
    
    let instruction_signature: &[u8] = &data[sig_start..sig_end];
    require!(instruction_signature == expected_signature, LpBondsError::InvalidOracleSignature);
    
    let msg_start = offsets.message_offset as usize;
    let msg_end = msg_start + offsets.message_data_size as usize;
    require!(data.len() >= msg_end, LpBondsError::InvalidOracleSignature);
    
    let instruction_message: &[u8] = &data[msg_start..msg_end];
    require!(instruction_message == expected_message, LpBondsError::MessageReconstructionFailed);
    
    msg!("Ed25519 signature verification: PASSED");
    Ok(())
}

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

/// Canonical message parameters for oracle verification.
#[derive(Debug, Clone)]
pub struct CanonicalMessageParams {
    pub bond_mint: Pubkey,
    pub position_mint: Pubkey,
    pub amount0: u64,
    pub amount1: u64,
    pub liquidity: u128,
    pub tick_lower: i32,
    pub tick_upper: i32,
    pub tick_current: i32,
    pub nonce: u64,
    pub sender: Pubkey,
    pub contract_address: Pubkey,
}

/// Reconstructs the canonical message for signature verification.
pub fn reconstruct_canonical_message(params: &CanonicalMessageParams) -> [u8; CANONICAL_MESSAGE_LEN] {
    let mut message = [0u8; CANONICAL_MESSAGE_LEN];
    let mut offset = 0;
    
    message[offset..offset + SIGNATURE_DOMAIN.len()].copy_from_slice(SIGNATURE_DOMAIN);
    offset += SIGNATURE_DOMAIN.len();
    
    message[offset..offset + 32].copy_from_slice(&params.bond_mint.to_bytes());
    offset += 32;
    
    message[offset..offset + 32].copy_from_slice(&params.position_mint.to_bytes());
    offset += 32;
    
    message[offset..offset + 8].copy_from_slice(&params.amount0.to_le_bytes());
    offset += 8;
    
    message[offset..offset + 8].copy_from_slice(&params.amount1.to_le_bytes());
    offset += 8;
    
    message[offset..offset + 16].copy_from_slice(&params.liquidity.to_le_bytes());
    offset += 16;
    
    message[offset..offset + 4].copy_from_slice(&params.tick_lower.to_le_bytes());
    offset += 4;
    
    message[offset..offset + 4].copy_from_slice(&params.tick_upper.to_le_bytes());
    offset += 4;
    
    message[offset..offset + 4].copy_from_slice(&params.tick_current.to_le_bytes());
    offset += 4;
    
    message[offset..offset + 8].copy_from_slice(&params.nonce.to_le_bytes());
    offset += 8;
    
    message[offset..offset + 32].copy_from_slice(&params.sender.to_bytes());
    offset += 32;
    
    message[offset..offset + 32].copy_from_slice(&params.contract_address.to_bytes());
    
    message
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_canonical_message_length() {
        let params = CanonicalMessageParams {
            bond_mint: Pubkey::default(),
            position_mint: Pubkey::default(),
            amount0: 0,
            amount1: 0,
            liquidity: 0,
            tick_lower: 0,
            tick_upper: 0,
            tick_current: 0,
            nonce: 0,
            sender: Pubkey::default(),
            contract_address: Pubkey::default(),
        };
        
        let message = reconstruct_canonical_message(&params);
        assert_eq!(message.len(), CANONICAL_MESSAGE_LEN);
    }
}
