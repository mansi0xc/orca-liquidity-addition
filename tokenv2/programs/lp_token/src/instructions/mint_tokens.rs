use crate::*;
use anchor_spl::token::{self, Mint, MintTo, Token, TokenAccount};

/// Mint new LP tokens to a recipient token account.
///
/// EVM equivalent: LPToken.mint(address _account, uint256 _amount)
///
/// Access control:
///   - Caller must be the registered `owner` OR a registered minter
///     (MinterRecord PDA with is_active == true)
///
/// Guards:
///   - Reverts if token_state.is_paused == true (matches whenNotPaused)
///   - No max supply cap (LPToken has none)
///
/// The `token_state` PDA signs the CPI call as the mint_authority.
#[derive(Accounts)]
pub struct MintTokens<'info> {
    /// Caller — must be owner or a registered minter
    pub authority: Signer<'info>,

    /// Program governance state.
    /// Seeds bind this to the specific mint.
    /// Constraint enforces the pause guard.
    #[account(
        seeds = [TOKEN_STATE_SEED, token_mint.key().as_ref()],
        bump = token_state.bump,
        constraint = !token_state.is_paused @ LPTokenError::Paused,
    )]
    pub token_state: Account<'info, TokenState>,

    /// MinterRecord PDA for the calling authority.
    /// Must be passed even if the caller is the owner (pass any valid pubkey;
    /// it will not be read if the caller is the owner).
    /// When the caller is a minter (not owner), this MUST be the correct PDA
    /// at seeds = ["minter", token_state, authority] with is_active == true.
    ///
    /// CHECK: PDA derivation and is_active check are performed in apply()
    pub minter_record: UncheckedAccount<'info>,

    /// The SPL mint to issue tokens from.
    #[account(mut)]
    pub token_mint: Account<'info, Mint>,

    /// Destination token account.
    /// Must belong to the SPL mint above.
    #[account(
        mut,
        constraint = recipient_token_account.mint == token_mint.key() @ LPTokenError::InvalidMint,
    )]
    pub recipient_token_account: Account<'info, TokenAccount>,

    pub token_program: Program<'info, Token>,
}

impl MintTokens<'_> {
    pub fn apply(ctx: &mut Context<MintTokens>, amount: u64) -> Result<()> {
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

        // --- CPI: mint_to, signed by token_state PDA ---
        let mint_key = ctx.accounts.token_mint.key();
        let seeds: &[&[u8]] = &[
            TOKEN_STATE_SEED,
            mint_key.as_ref(),
            &[ctx.accounts.token_state.bump],
        ];
        let signer_seeds = &[seeds];

        token::mint_to(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                MintTo {
                    mint: ctx.accounts.token_mint.to_account_info(),
                    to: ctx.accounts.recipient_token_account.to_account_info(),
                    authority: ctx.accounts.token_state.to_account_info(),
                },
                signer_seeds,
            ),
            amount,
        )?;

        emit!(events::TokensMinted {
            authority: ctx.accounts.authority.key(),
            recipient: ctx.accounts.recipient_token_account.key(),
            amount,
        });

        Ok(())
    }
}

/// Verify that the passed `minter_record_info` is the correct PDA for the
/// given (token_state, authority) pair and that is_active == true.
///
/// Uses manual deserialization via UncheckedAccount to support the case where
/// the caller may be either the owner (no MinterRecord needed) or a minter.
pub fn verify_minter(
    minter_record_info: &UncheckedAccount,
    token_state: &Account<TokenState>,
    authority: &Signer,
    program_id: &Pubkey,
) -> Result<()> {
    // Derive the expected PDA address
    let (expected_pda, _bump) = Pubkey::find_program_address(
        &[
            MINTER_SEED,
            token_state.key().as_ref(),
            authority.key().as_ref(),
        ],
        program_id,
    );

    require!(
        minter_record_info.key() == expected_pda,
        LPTokenError::Unauthorized
    );

    // Account must be initialized (non-empty data)
    require!(
        !minter_record_info.data_is_empty(),
        LPTokenError::Unauthorized
    );

    // Deserialize and check is_active (includes discriminator verification)
    let data = minter_record_info.try_borrow_data()?;
    let mut reader: &[u8] = &data;
    let record = MinterRecord::try_deserialize(&mut reader)
        .map_err(|_| error!(LPTokenError::Unauthorized))?;

    require!(record.is_active, LPTokenError::Unauthorized);

    Ok(())
}
