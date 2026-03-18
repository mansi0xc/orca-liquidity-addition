use anchor_lang::prelude::*;
use crate::state::{Collection, WhitelistEntry};
use crate::errors::LaunchpadError;
use crate::events::WhitelistAdded;

#[derive(Accounts)]
pub struct AddWhitelist<'info> {
    #[account(
        seeds = [Collection::SEED_PREFIX, collection.collection_id.as_ref()],
        bump = collection.bump,
        has_one = authority @ LaunchpadError::Unauthorized,
    )]
    pub collection: Account<'info, Collection>,

    #[account(
        init,
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

    /// CHECK: The user to whitelist
    #[account(
        constraint = user.key() != Pubkey::default() @ LaunchpadError::InvalidUserAddress,
    )]
    pub user: AccountInfo<'info>,

    #[account(mut)]
    pub authority: Signer<'info>,

    pub system_program: Program<'info, System>,
}

pub fn handler(ctx: Context<AddWhitelist>, mint_limit: u64) -> Result<()> {
    let entry = &mut ctx.accounts.whitelist_entry;
    entry.collection = ctx.accounts.collection.key();
    entry.user = ctx.accounts.user.key();
    entry.mint_limit = mint_limit; // This is the per-user presale mint cap (EVM IB2)
    entry.bump = ctx.bumps.whitelist_entry;

    emit!(WhitelistAdded {
        collection: ctx.accounts.collection.key(),
        user: ctx.accounts.user.key(),
        mint_limit,
    });

    Ok(())
}
