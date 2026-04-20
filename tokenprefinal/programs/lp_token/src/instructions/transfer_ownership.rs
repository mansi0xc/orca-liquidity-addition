use crate::*;
use anchor_spl::token::Mint;

/// Propose a new owner for the token program (step 1 of 2).
///
/// EVM equivalent: OwnableUpgradeable.transferOwnership(address newOwner)
///
/// Production improvement over EVM: uses a two-step propose/accept pattern
/// (similar to OpenZeppelin Ownable2Step) to prevent accidental ownership
/// loss from typos or incorrect addresses. The EVM contract performs an
/// instant one-step transfer which is irreversible.
///
/// Access control: only the current owner may call this.
///
/// The proposed new_owner must call `accept_ownership` to finalize.
/// The current owner can overwrite the pending owner at any time by
/// calling this again, or cancel by passing Pubkey::default().
///
/// Note on renounceOwnership:
///   The EVM OwnableUpgradeable also exposes renounceOwnership() which sets
///   owner to address(0). This is intentionally NOT implemented on Solana.
///   Renouncing ownership is irreversible and would permanently disable
///   minter management, pause control, and ownership transfer.
///   The LP bond use case requires ongoing governance. Blocking renunciation
///   is a conscious safety restriction over the EVM contract.
#[derive(Accounts)]
pub struct TransferOwnership<'info> {
    /// Must be the current registered owner.
    pub owner: Signer<'info>,

    /// Program governance state.
    #[account(
        mut,
        seeds = [TOKEN_STATE_SEED, token_mint.key().as_ref()],
        bump = token_state.bump,
        constraint = owner.key() == token_state.owner @ LPTokenError::Unauthorized,
    )]
    pub token_state: Account<'info, TokenState>,

    /// The SPL mint — used to derive and verify the token_state PDA.
    pub token_mint: Account<'info, Mint>,
}

impl TransferOwnership<'_> {
    pub fn apply(ctx: &mut Context<TransferOwnership>, new_owner: Pubkey) -> Result<()> {
        // Match EVM OwnableUpgradeable: require(newOwner != address(0))
        require!(
            new_owner != Pubkey::default(),
            LPTokenError::InvalidOwner
        );

        // Don't allow proposing the current owner (no-op transfer)
        require!(
            new_owner != ctx.accounts.token_state.owner,
            LPTokenError::InvalidOwner
        );

        ctx.accounts.token_state.pending_owner = new_owner;

        emit!(events::OwnershipTransferProposed {
            current_owner: ctx.accounts.owner.key(),
            proposed_owner: new_owner,
        });

        Ok(())
    }
}
