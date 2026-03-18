use anchor_lang::prelude::*;
use crate::state::{Collection, WhitelistEntry};
use crate::errors::LaunchpadError;

#[derive(Accounts)]
pub struct RemoveWhitelist<'info> {
    #[account(
        seeds = [Collection::SEED_PREFIX, collection.collection_id.as_ref()],
        bump = collection.bump,
        constraint = authority.key() == collection.authority @ LaunchpadError::Unauthorized,
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
        constraint = whitelist_entry.collection == collection.key(),
    )]
    pub whitelist_entry: Account<'info, WhitelistEntry>,

    /// CHECK: The user being removed
    pub user: AccountInfo<'info>,

    #[account(mut)]
    pub authority: Signer<'info>,
}

pub fn handler(_ctx: Context<RemoveWhitelist>) -> Result<()> {
    // Handled by close = authority
    Ok(())
}
