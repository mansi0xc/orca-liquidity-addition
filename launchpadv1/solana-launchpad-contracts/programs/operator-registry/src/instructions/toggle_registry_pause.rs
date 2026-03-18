use anchor_lang::prelude::*;
use crate::state::OperatorRegistryState;
use crate::errors::RegistryError;
use crate::events::RegistryPauseToggled;

#[derive(Accounts)]
pub struct ToggleRegistryPause<'info> {
    #[account(
        mut,
        seeds = [OperatorRegistryState::SEED_PREFIX],
        bump = registry.bump,
        has_one = authority @ RegistryError::Unauthorized,
    )]
    pub registry: Account<'info, OperatorRegistryState>,

    pub authority: Signer<'info>,
}

pub fn handler(ctx: Context<ToggleRegistryPause>) -> Result<()> {
    let registry = &mut ctx.accounts.registry;
    registry.paused = !registry.paused;

    emit!(RegistryPauseToggled {
        paused: registry.paused,
    });

    Ok(())
}
