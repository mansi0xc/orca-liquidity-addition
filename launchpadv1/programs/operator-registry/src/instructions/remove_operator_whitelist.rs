use anchor_lang::prelude::*;
use crate::state::{OperatorRegistryState, OperatorWhitelist};
use crate::errors::RegistryError;
use crate::events::OperatorWhitelistRemoved;

#[derive(Accounts)]
pub struct RemoveOperatorWhitelist<'info> {
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
            OperatorWhitelist::SEED_PREFIX,
            collection.key().as_ref(),
            operator.key().as_ref(),
        ],
        bump = whitelist_entry.bump,
        constraint = whitelist_entry.is_allowed @ RegistryError::NotWhitelisted,
    )]
    pub whitelist_entry: Account<'info, OperatorWhitelist>,

    /// CHECK: The collection address
    pub collection: UncheckedAccount<'info>,

    /// CHECK: The operator address
    pub operator: UncheckedAccount<'info>,

    #[account(mut)]
    pub authority: Signer<'info>,
}

pub fn handler(ctx: Context<RemoveOperatorWhitelist>) -> Result<()> {
    require!(!ctx.accounts.registry.paused, RegistryError::RegistryPaused);

    emit!(OperatorWhitelistRemoved {
        collection: ctx.accounts.collection.key(),
        operator: ctx.accounts.operator.key(),
    });

    // Account is closed via the `close = authority` constraint

    Ok(())
}
