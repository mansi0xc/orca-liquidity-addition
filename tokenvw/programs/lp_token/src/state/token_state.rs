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

    /// Pending new owner for two-step ownership transfer.
    /// Set by `transfer_ownership`, cleared by `accept_ownership` or
    /// by a new `transfer_ownership` call.
    /// Pubkey::default() means no pending transfer.
    ///
    /// Improvement over EVM: OwnableUpgradeable uses a one-step transfer
    /// where a typo permanently loses governance. This two-step pattern
    /// (similar to OpenZeppelin Ownable2Step) requires the new owner to
    /// actively accept, preventing accidental loss.
    pub pending_owner: Pubkey,

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
