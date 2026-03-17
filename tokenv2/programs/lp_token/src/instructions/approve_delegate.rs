use crate::*;
use anchor_spl::token::{self, Approve, Token, TokenAccount};

/// Set a delegate authority on a token account with an allowance.
///
/// EVM equivalent: ERC20.approve(address spender, uint256 amount)
///
/// No custom logic: LPToken.sol does NOT override _approve.
/// Approval is not blocked by pause. This instruction is a documented
/// wrapper around the standard SPL approve CPI.
///
/// After calling this, the delegate may call transfer_tokens (or SPL transfer
/// directly) up to `amount` tokens on behalf of the token account owner.
/// This maps to the ERC20 transferFrom pattern.
#[derive(Accounts)]
pub struct ApproveDelegate<'info> {
    /// Owner of the token account granting the approval.
    pub token_account_owner: Signer<'info>,

    /// Token account on which the delegate is being set.
    #[account(
        mut,
        constraint = token_account.owner == token_account_owner.key() @ LPTokenError::InvalidTokenAuthority,
    )]
    pub token_account: Account<'info, TokenAccount>,

    /// The delegate being authorized.
    /// CHECK: Any pubkey can be a delegate.
    pub delegate: UncheckedAccount<'info>,

    pub token_program: Program<'info, Token>,
}

impl ApproveDelegate<'_> {
    pub fn apply(ctx: &mut Context<ApproveDelegate>, amount: u64) -> Result<()> {
        // No custom guards — LPToken has no _approve override.
        // Pause does NOT block approvals (matching EVM LPToken behavior).
        token::approve(
            CpiContext::new(
                ctx.accounts.token_program.to_account_info(),
                Approve {
                    to: ctx.accounts.token_account.to_account_info(),
                    delegate: ctx.accounts.delegate.to_account_info(),
                    authority: ctx.accounts.token_account_owner.to_account_info(),
                },
            ),
            amount,
        )?;

        Ok(())
    }
}
