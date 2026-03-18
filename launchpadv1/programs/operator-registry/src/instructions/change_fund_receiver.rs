use anchor_lang::prelude::*;
use crate::state::OperatorRegistryState;
use crate::errors::RegistryError;
use crate::events::FundReceiverChanged;

#[derive(Accounts)]
pub struct ChangeFundReceiver<'info> {
    #[account(
        mut,
        seeds = [OperatorRegistryState::SEED_PREFIX],
        bump = registry.bump,
        has_one = authority @ RegistryError::Unauthorized,
    )]
    pub registry: Account<'info, OperatorRegistryState>,

    pub authority: Signer<'info>,
}

pub fn handler(ctx: Context<ChangeFundReceiver>, new_fund_receiver: Pubkey) -> Result<()> {
    require!(!ctx.accounts.registry.paused, RegistryError::RegistryPaused);
    require!(
        new_fund_receiver != Pubkey::default(),
        RegistryError::InvalidFundReceiver
    );
    require!(
        new_fund_receiver != ctx.accounts.registry.fund_receiver,
        RegistryError::FundReceiverAlreadySet
    );

    let old_receiver = ctx.accounts.registry.fund_receiver;
    ctx.accounts.registry.fund_receiver = new_fund_receiver;

    emit!(FundReceiverChanged {
        old_receiver,
        new_receiver: new_fund_receiver,
    });

    Ok(())
}
