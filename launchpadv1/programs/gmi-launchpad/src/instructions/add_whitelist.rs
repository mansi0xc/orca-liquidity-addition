use anchor_lang::prelude::*;
use crate::state::{Collection, WhitelistEntry};
use crate::errors::LaunchpadError;

#[derive(Accounts)]
pub struct AddWhitelist<'info> {
    #[account(
        seeds = [Collection::SEED_PREFIX, collection.collection_id.as_ref()],
        bump = collection.bump,
        constraint = authority.key() == collection.authority @ LaunchpadError::Unauthorized,
    )]
    pub collection: Account<'info, Collection>,
    
    #[account(
        init_if_needed,
        payer = authority,
        space = WhitelistEntry::SIZE,
        seeds = [
            WhitelistEntry::SEED_PREFIX,
            collection.key().as_ref(),
            user.key().as_ref(),
        ],
        bump,
    )]
    pub whitelist_entry: Account<'info, WhitelistEntry>,

    /// CHECK: The user being whitelisted
    pub user: AccountInfo<'info>,

    #[account(mut)]
    pub authority: Signer<'info>,
    
    pub system_program: Program<'info, System>,
}

pub fn handler(ctx: Context<AddWhitelist>, mint_limit: u64) -> Result<()> {
    let entry = &mut ctx.accounts.whitelist_entry;
    if entry.collection == Pubkey::default() {
        entry.collection = ctx.accounts.collection.key();
        entry.user = ctx.accounts.user.key();
        entry.bump = ctx.bumps.whitelist_entry;
    }
    entry.mint_limit = mint_limit;
    Ok(())
}
