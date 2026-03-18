use anchor_lang::prelude::*;
use crate::state::OperatorRegistryState;
use crate::errors::RegistryError;
use crate::events::SharePercentageBpsChanged;

#[derive(Accounts)]
pub struct ChangeSharePercentage<'info> {
    #[account(
        mut,
        seeds = [OperatorRegistryState::SEED_PREFIX],
        bump = registry.bump,
        has_one = authority @ RegistryError::Unauthorized,
    )]
    pub registry: Account<'info, OperatorRegistryState>,

    pub authority: Signer<'info>,
}

pub fn handler(ctx: Context<ChangeSharePercentage>, new_share_percentage_bps: u64) -> Result<()> {
    require!(!ctx.accounts.registry.paused, RegistryError::RegistryPaused);
    require!(
        new_share_percentage_bps <= 10_000,
        RegistryError::SharePercentageTooHigh
    );

    let old_percentage = ctx.accounts.registry.share_percentage_bps;
    ctx.accounts.registry.share_percentage_bps = new_share_percentage_bps;

    emit!(SharePercentageBpsChanged {
        old_share_percentage: old_percentage,
        new_share_percentage: new_share_percentage_bps,
    });

    Ok(())
}
