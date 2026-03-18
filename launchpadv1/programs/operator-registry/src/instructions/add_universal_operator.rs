use anchor_lang::prelude::*;
use crate::state::{OperatorRegistryState, UniversalOperator};
use crate::errors::RegistryError;
use crate::events::UniversalOperatorAdded;

#[derive(Accounts)]
pub struct AddUniversalOperator<'info> {
    #[account(
        seeds = [OperatorRegistryState::SEED_PREFIX],
        bump = registry.bump,
        has_one = authority @ RegistryError::Unauthorized,
    )]
    pub registry: Account<'info, OperatorRegistryState>,

    #[account(
        init,
        payer = authority,
        space = UniversalOperator::SIZE,
        seeds = [
            UniversalOperator::SEED_PREFIX,
            operator.key().as_ref(),
        ],
        bump,
    )]
    pub universal_entry: Account<'info, UniversalOperator>,

    /// CHECK: The operator (marketplace) address
    pub operator: UncheckedAccount<'info>,

    #[account(mut)]
    pub authority: Signer<'info>,

    pub system_program: Program<'info, System>,
}

pub fn handler(ctx: Context<AddUniversalOperator>) -> Result<()> {
    require!(!ctx.accounts.registry.paused, RegistryError::RegistryPaused);

    let entry = &mut ctx.accounts.universal_entry;
    entry.operator = ctx.accounts.operator.key();
    entry.is_allowed = true;
    entry.bump = ctx.bumps.universal_entry;

    emit!(UniversalOperatorAdded {
        operator: ctx.accounts.operator.key(),
    });

    Ok(())
}
