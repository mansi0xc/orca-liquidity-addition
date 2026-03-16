use anchor_lang::prelude::*;

/// Program control account for the LP token.
///
/// Holds the governance state for one SPL mint.
/// Acts as the mint_authority and freeze_authority on the SPL mint,
/// allowing the program to CPI into token::mint_to using PDA signing.
///
/// Seeds: ["token_state", mint_pubkey]
#[account]
#[derive(InitSpace)]
pub struct TokenState {
    /// The admin with exclusive governance rights.
    /// Maps to EVM: OwnableUpgradeable.owner
    pub owner: Pubkey,

    /// Whether mint and burn operations are currently blocked.
    /// Maps to EVM: PausableUpgradeable.paused
    /// Note: Regular SPL transfers are NOT blocked by this flag,
    /// matching LPToken.sol behavior (no _transfer override).
    pub is_paused: bool,

    /// EVM chain ID this LP token corresponds to.
    /// Maps to EVM: LPToken.chainId
    pub evm_chain_id: u64,

    /// PDA bump seed used for signing CPI calls.
    pub bump: u8,
}
