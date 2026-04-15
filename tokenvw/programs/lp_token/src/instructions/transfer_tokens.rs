use crate::*;
use anchor_spl::token::{self, Token, TokenAccount, Transfer};

/// Transfer LP tokens between accounts.
///
/// EVM equivalent: ERC20.transfer(address to, uint256 amount)
///                 ERC20.transferFrom(address from, address to, uint256 amount)
///
/// No custom logic: LPToken.sol does NOT override _transfer.
/// Transfers are not blocked by pause. This instruction is a documented
/// wrapper around the standard SPL transfer CPI.
///
/// For delegated transfers (transferFrom equivalent), the caller should be
/// the delegate set via approve_delegate, and SPL Token will enforce the
/// delegated_amount allowance.
#[derive(Accounts)]
pub struct TransferTokens<'info> {
    /// Owner of the source token account (or delegate).
    pub from_authority: Signer<'info>,

    /// Source token account.
    #[account(mut)]
    pub from_token_account: Account<'info, TokenAccount>,

    /// Destination token account.
    #[account(
        mut,
        constraint = to_token_account.mint == from_token_account.mint @ LPTokenError::InvalidMint,
    )]
    pub to_token_account: Account<'info, TokenAccount>,

    pub token_program: Program<'info, Token>,
}

impl TransferTokens<'_> {
    pub fn apply(ctx: &mut Context<TransferTokens>, amount: u64) -> Result<()> {
        // No custom guards — LPToken has no _transfer override.
        // Pause does NOT block transfers (matching EVM LPToken behavior).
        token::transfer(
            CpiContext::new(
                ctx.accounts.token_program.to_account_info(),
                Transfer {
                    from: ctx.accounts.from_token_account.to_account_info(),
                    to: ctx.accounts.to_token_account.to_account_info(),
                    authority: ctx.accounts.from_authority.to_account_info(),
                },
            ),
            amount,
        )?;

        Ok(())
    }
}
