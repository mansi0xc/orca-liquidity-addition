use anchor_lang::prelude::*;
use crate::state::{OperatorRegistryState, OperatorWhitelist};
use crate::errors::RegistryError;
use crate::events::OperatorWhitelistAdded;

#[derive(Accounts)]
#[instruction()]
pub struct AddOperatorWhitelist<'info> {
    #[account(
        seeds = [OperatorRegistryState::SEED_PREFIX],
        bump = registry.bump,
        has_one = authority @ RegistryError::Unauthorized,
    )]
    pub registry: Account<'info, OperatorRegistryState>,

    #[account(
        init,
        payer = authority,
        space = OperatorWhitelist::SIZE,
        seeds = [
            OperatorWhitelist::SEED_PREFIX,
            collection.key().as_ref(),
            operator.key().as_ref(),
        ],
        bump,
    )]
    pub whitelist_entry: Account<'info, OperatorWhitelist>,

    /// CHECK: The collection address to whitelist for
    pub collection: UncheckedAccount<'info>,

    /// CHECK: The operator (marketplace) address to whitelist
    pub operator: UncheckedAccount<'info>,

    #[account(mut)]
    pub authority: Signer<'info>,

    pub system_program: Program<'info, System>,
}

pub fn handler(ctx: Context<AddOperatorWhitelist>) -> Result<()> {
    require!(!ctx.accounts.registry.paused, RegistryError::RegistryPaused);

    let entry = &mut ctx.accounts.whitelist_entry;
    entry.collection = ctx.accounts.collection.key();
    entry.operator = ctx.accounts.operator.key();
    entry.is_allowed = true;
    entry.bump = ctx.bumps.whitelist_entry;

    emit!(OperatorWhitelistAdded {
        collection: ctx.accounts.collection.key(),
        operator: ctx.accounts.operator.key(),
    });

    Ok(())
}
