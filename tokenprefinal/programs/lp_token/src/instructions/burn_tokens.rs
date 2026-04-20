use crate::*;
use crate::instructions::mint_tokens::verify_minter;
use anchor_spl::token::{self, Burn, Mint, Token, TokenAccount};

/// Burn LP tokens from a token account.
///
/// EVM equivalent: LPToken.burn(address _account, uint256 _amount)
///
/// Access control:
///   - `authority` must be the registered owner OR a registered minter
///   - `token_account_authority` must own the token account being burned from
///
/// Security note — behavioral difference from EVM:
///   In the EVM LPToken, a minter can burn from any address without that
///   address's consent. On Solana, the token account holder must also sign.
///   This is a security improvement: it prevents minters from draining
///   user wallets without user interaction.
///   In the LP bond use case, the user always signs when redeeming a position,
///   so this dual-signer requirement does not break the intended workflow.
///
/// Guards:
///   - Reverts if token_state.is_paused == true (matches whenNotPaused)
#[derive(Accounts)]
pub struct BurnTokens<'info> {
    /// The authority initiating the burn — must be owner or registered minter.
    pub authority: Signer<'info>,

    /// The owner of the token account being burned from.
    /// Must sign to authorize the burn (Solana security improvement over EVM).
    pub token_account_authority: Signer<'info>,

    /// Program governance state.
    #[account(
        seeds = [TOKEN_STATE_SEED, token_mint.key().as_ref()],
        bump = token_state.bump,
        constraint = !token_state.is_paused @ LPTokenError::Paused,
    )]
    pub token_state: Account<'info, TokenState>,

    /// MinterRecord PDA for the calling authority.
    /// Must be provided and valid when authority is not the owner.
    ///
    /// CHECK: PDA derivation and is_active check performed in apply()
    pub minter_record: UncheckedAccount<'info>,

    /// The SPL mint.
    /// Defense-in-depth: verify the mint_authority is our token_state PDA.
    #[account(
        mut,
        constraint = token_mint.mint_authority.contains(&token_state.key()) @ LPTokenError::InvalidMint,
    )]
    pub token_mint: Account<'info, Mint>,

    /// The token account to burn from.
    #[account(
        mut,
        constraint = token_account.mint == token_mint.key() @ LPTokenError::InvalidMint,
        constraint = token_account.owner == token_account_authority.key() @ LPTokenError::InvalidTokenAuthority,
    )]
    pub token_account: Account<'info, TokenAccount>,

    pub token_program: Program<'info, Token>,
}

impl BurnTokens<'_> {
    pub fn apply(ctx: &mut Context<BurnTokens>, amount: u64) -> Result<()> {
        // --- Access control: owner OR registered minter ---
        let is_owner = ctx.accounts.authority.key() == ctx.accounts.token_state.owner;
        if !is_owner {
            verify_minter(
                &ctx.accounts.minter_record,
                &ctx.accounts.token_state,
                &ctx.accounts.authority,
                ctx.program_id,
            )?;
        }

        // --- CPI: burn, signed by the token account authority ---
        // The token_account_authority must be a Signer so SPL Token accepts the burn.
        token::burn(
            CpiContext::new(
                ctx.accounts.token_program.to_account_info(),
                Burn {
                    mint: ctx.accounts.token_mint.to_account_info(),
                    from: ctx.accounts.token_account.to_account_info(),
                    authority: ctx.accounts.token_account_authority.to_account_info(),
                },
            ),
            amount,
        )?;

        emit!(events::TokensBurned {
            authority: ctx.accounts.authority.key(),
            from: ctx.accounts.token_account.key(),
            amount,
        });

        Ok(())
    }
}
