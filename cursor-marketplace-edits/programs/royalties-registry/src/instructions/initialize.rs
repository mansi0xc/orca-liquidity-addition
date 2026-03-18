use anchor_lang::prelude::*;

use crate::state::RegistryConfig;

#[derive(Accounts)]
pub struct InitializeRegistry<'info> {
    #[account(
        init,
        payer = authority,
        space = 8 + RegistryConfig::INIT_SPACE,
        seeds = [b"registry_config"],
        bump,
    )]
    pub registry_config: Account<'info, RegistryConfig>,

    #[account(mut)]
    pub authority: Signer<'info>,

    pub system_program: Program<'info, System>,
}

pub fn handler(ctx: Context<InitializeRegistry>) -> Result<()> {
    let config = &mut ctx.accounts.registry_config;
    config.owner = ctx.accounts.authority.key();
    config.bump = ctx.bumps.registry_config;
    Ok(())
}
