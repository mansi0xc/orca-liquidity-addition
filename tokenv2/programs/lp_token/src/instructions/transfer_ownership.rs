use crate::*;
use anchor_spl::token::Mint;

/// Transfer ownership of the token program to a new address.
///
/// EVM equivalent: OwnableUpgradeable.transferOwnership(address newOwner)
///
/// Access control: only the current owner may call this.
///
/// This is a critical governance operation. The new owner gains full control
/// over pause state, minter management, and ownership transfer.
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
        ctx.accounts.token_state.owner = new_owner;

        emit!(events::OwnershipTransferred {
            previous_owner: ctx.accounts.owner.key(),
            new_owner,
        });

        Ok(())
    }
}
