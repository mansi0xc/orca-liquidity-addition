use anchor_lang::prelude::*;
use crate::state::{Collection, WhitelistEntry};
use crate::errors::LaunchpadError;
use crate::events::WhitelistRemoved;

#[derive(Accounts)]
pub struct RemoveWhitelist<'info> {
    #[account(
        seeds = [Collection::SEED_PREFIX, collection.collection_id.as_ref()],
        bump = collection.bump,
        has_one = authority @ LaunchpadError::Unauthorized,
    )]
    pub collection: Account<'info, Collection>,

    #[account(
        mut,
        close = authority,
        seeds = [
            WhitelistEntry::SEED_PREFIX,
            collection.key().as_ref(),
            user.key().as_ref(),
        ],
        bump = whitelist_entry.bump,
        constraint = whitelist_entry.collection == collection.key() @ LaunchpadError::NotWhitelisted,
    )]
    pub whitelist_entry: Account<'info, WhitelistEntry>,

    /// CHECK: The user to remove from whitelist
    #[account(
        constraint = user.key() != Pubkey::default() @ LaunchpadError::InvalidUserAddress,
    )]
    pub user: AccountInfo<'info>,

    #[account(mut)]
    pub authority: Signer<'info>,
}

pub fn handler(ctx: Context<RemoveWhitelist>) -> Result<()> {
    emit!(WhitelistRemoved {
        collection: ctx.accounts.collection.key(),
        user: ctx.accounts.user.key(),
    });

    // Account is closed via the `close = authority` constraint, returning rent

    Ok(())
}
