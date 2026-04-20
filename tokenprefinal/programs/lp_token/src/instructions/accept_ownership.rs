use crate::*;
use anchor_spl::token::Mint;

/// Accept a pending ownership transfer (step 2 of 2).
///
/// No direct EVM equivalent — this is a production security improvement.
/// The EVM OwnableUpgradeable uses one-step transfer; this two-step pattern
/// prevents accidental ownership loss.
///
/// Access control: only the pending_owner may call this.
///
/// After this call:
///   - token_state.owner = pending_owner
///   - token_state.pending_owner = Pubkey::default() (cleared)
///   - The previous owner loses all governance powers immediately
#[derive(Accounts)]
pub struct AcceptOwnership<'info> {
    /// Must be the pending owner proposed by transfer_ownership.
    pub new_owner: Signer<'info>,

    /// Program governance state.
    #[account(
        mut,
        seeds = [TOKEN_STATE_SEED, token_mint.key().as_ref()],
        bump = token_state.bump,
        constraint = new_owner.key() == token_state.pending_owner @ LPTokenError::NoPendingOwnership,
    )]
    pub token_state: Account<'info, TokenState>,

    /// The SPL mint — used to derive and verify the token_state PDA.
    pub token_mint: Account<'info, Mint>,
}

impl AcceptOwnership<'_> {
    pub fn apply(ctx: &mut Context<AcceptOwnership>) -> Result<()> {
        let previous_owner = ctx.accounts.token_state.owner;
        let new_owner = ctx.accounts.new_owner.key();

        ctx.accounts.token_state.owner = new_owner;
        ctx.accounts.token_state.pending_owner = Pubkey::default();

        emit!(events::OwnershipTransferred {
            previous_owner,
            new_owner,
        });

        Ok(())
    }
}
