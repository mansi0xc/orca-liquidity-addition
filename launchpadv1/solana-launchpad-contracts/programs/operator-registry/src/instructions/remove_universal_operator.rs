use anchor_lang::prelude::*;
use crate::state::{OperatorRegistryState, UniversalOperator};
use crate::errors::RegistryError;
use crate::events::UniversalOperatorRemoved;

#[derive(Accounts)]
pub struct RemoveUniversalOperator<'info> {
    #[account(
        seeds = [OperatorRegistryState::SEED_PREFIX],
        bump = registry.bump,
        has_one = authority @ RegistryError::Unauthorized,
    )]
    pub registry: Account<'info, OperatorRegistryState>,

    #[account(
        mut,
        close = authority,
        seeds = [
            UniversalOperator::SEED_PREFIX,
            operator.key().as_ref(),
        ],
        bump = universal_entry.bump,
        constraint = universal_entry.is_allowed @ RegistryError::NotWhitelisted,
    )]
    pub universal_entry: Account<'info, UniversalOperator>,

    /// CHECK: The operator address
    pub operator: UncheckedAccount<'info>,

    #[account(mut)]
    pub authority: Signer<'info>,
}

pub fn handler(ctx: Context<RemoveUniversalOperator>) -> Result<()> {
    require!(!ctx.accounts.registry.paused, RegistryError::RegistryPaused);

    emit!(UniversalOperatorRemoved {
        operator: ctx.accounts.operator.key(),
    });

    Ok(())
}
