use anchor_lang::prelude::*;
use crate::state::OperatorRegistryState;
use crate::errors::RegistryError;

#[derive(Accounts)]
pub struct InitializeRegistry<'info> {
    #[account(
        init,
        payer = authority,
        space = OperatorRegistryState::SIZE,
        seeds = [OperatorRegistryState::SEED_PREFIX],
        bump,
    )]
    pub registry: Account<'info, OperatorRegistryState>,

    #[account(mut)]
    pub authority: Signer<'info>,

    pub system_program: Program<'info, System>,
}

pub fn handler(
    ctx: Context<InitializeRegistry>,
    fund_receiver: Pubkey,
    share_percentage_bps: u64,
) -> Result<()> {
    require!(
        fund_receiver != Pubkey::default(),
        RegistryError::InvalidFundReceiver
    );
    require!(
        share_percentage_bps <= 10_000,
        RegistryError::SharePercentageTooHigh
    );

    let registry = &mut ctx.accounts.registry;
    registry.authority = ctx.accounts.authority.key();
    registry.fund_receiver = fund_receiver;
    registry.share_percentage_bps = share_percentage_bps;
    registry.paused = false;
    registry.bump = ctx.bumps.registry;

    Ok(())
}
