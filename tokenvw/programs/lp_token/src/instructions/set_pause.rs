use crate::*;
use anchor_spl::token::Mint;

/// Pause or unpause the token mint/burn operations.
///
/// EVM equivalent:
///   pause()   → set_pause { paused: true }
///   unpause() → set_pause { paused: false }
///
/// Access control: only the owner (token_state.owner) may call this.
///
/// State transition guards:
///   pause()   is guarded by whenNotPaused — cannot pause when already paused
///   unpause() is guarded by whenPaused    — cannot unpause when not paused
///
/// Effect:
///   Only mint_tokens and burn_tokens are blocked while paused.
///   Regular SPL transfers are NOT blocked, matching LPToken.sol behavior
///   (LPToken does not override _transfer or _approve).
#[derive(Accounts)]
pub struct SetPause<'info> {
    /// Must be the registered owner.
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

impl SetPause<'_> {
    pub fn apply(ctx: &mut Context<SetPause>, paused: bool) -> Result<()> {
        let current = ctx.accounts.token_state.is_paused;

        // Guard: match EVM whenNotPaused / whenPaused transitions
        if paused {
            // Equivalent to whenNotPaused on pause()
            require!(!current, LPTokenError::InvalidPauseState);
        } else {
            // Equivalent to whenPaused on unpause()
            require!(current, LPTokenError::InvalidPauseState);
        }

        ctx.accounts.token_state.is_paused = paused;

        emit!(events::PauseStateChanged {
            paused,
            authority: ctx.accounts.owner.key(),
        });

        Ok(())
    }
}
