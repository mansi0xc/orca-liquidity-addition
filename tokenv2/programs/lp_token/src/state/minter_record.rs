use anchor_lang::prelude::*;

/// Per-minter authorization account.
///
/// One account exists per (token_state, minter) pair.
/// Maps to EVM: mapping(address => bool) public minters
///
/// Seeds: ["minter", token_state_pubkey, minter_pubkey]
#[account]
#[derive(InitSpace)]
pub struct MinterRecord {
    /// Whether this address is currently authorized to mint/burn.
    pub is_active: bool,

    /// The public key of the minter this record is for.
    /// Stored for reference; the PDA seeds already enforce uniqueness.
    pub minter: Pubkey,

    /// PDA bump seed.
    pub bump: u8,
}
