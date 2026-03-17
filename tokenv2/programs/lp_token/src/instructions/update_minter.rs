use crate::*;
use anchor_spl::token::Mint;

/// Register or deregister a minter address.
///
/// EVM equivalent: LPToken.updateMinter(address _account, bool _isMinter)
///
/// Access control: only the owner (token_state.owner) may call this.
///
/// Duplicate prevention: reverts if the requested state equals the current state,
/// matching the EVM require(minters[_account] != _isMinter, "Duplicate operation").
///
/// Creates the MinterRecord PDA on first call (init_if_needed).
/// On subsequent calls, updates the is_active field in place.
#[derive(Accounts)]
#[instruction(params: UpdateMinterParams)]
pub struct UpdateMinter<'info> {
    /// Must be the registered owner.
    #[account(mut)]
    pub owner: Signer<'info>,

    /// Program governance state — used to validate owner and derive minter PDA.
    #[account(
        seeds = [TOKEN_STATE_SEED, token_mint.key().as_ref()],
        bump = token_state.bump,
        constraint = owner.key() == token_state.owner @ LPTokenError::Unauthorized,
    )]
    pub token_state: Account<'info, TokenState>,

    /// The address being registered or deregistered as a minter.
    /// CHECK: This is only used as a seed component and stored in the record.
    pub target_minter: UncheckedAccount<'info>,

    /// MinterRecord PDA for the (token_state, target_minter) pair.
    /// Created with init_if_needed on first registration.
    #[account(
        init_if_needed,
        payer = owner,
        space = 8 + MinterRecord::INIT_SPACE,
        seeds = [MINTER_SEED, token_state.key().as_ref(), target_minter.key().as_ref()],
        bump,
    )]
    pub minter_record: Account<'info, MinterRecord>,

    /// The SPL mint this token_state is associated with.
    /// Typed as Account<Mint> to ensure it is a valid SPL mint.
    pub token_mint: Account<'info, Mint>,

    pub system_program: Program<'info, System>,
}

impl UpdateMinter<'_> {
    pub fn apply(ctx: &mut Context<UpdateMinter>, params: &UpdateMinterParams) -> Result<()> {
        // Duplicate prevention — matches EVM: require(minters[_account] != _isMinter)
        require!(
            ctx.accounts.minter_record.is_active != params.is_active,
            LPTokenError::DuplicateOperation
        );

        let record = &mut ctx.accounts.minter_record;
        record.is_active = params.is_active;
        record.minter = ctx.accounts.target_minter.key();
        record.bump = ctx.bumps.minter_record;

        emit!(events::MinterUpdated {
            minter: ctx.accounts.target_minter.key(),
            is_active: params.is_active,
        });

        Ok(())
    }
}

#[derive(Clone, AnchorSerialize, AnchorDeserialize)]
pub struct UpdateMinterParams {
    /// true to add minter, false to remove. Maps to EVM _isMinter.
    pub is_active: bool,
}
