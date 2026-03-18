use anchor_lang::prelude::*;
use crate::state::Collection;
use crate::errors::LaunchpadError;

#[derive(Accounts)]
pub struct TogglePause<'info> {
    #[account(
        mut,
        seeds = [Collection::SEED_PREFIX, collection.collection_id.as_ref()],
        bump = collection.bump,
        constraint = authority.key() == collection.authority @ LaunchpadError::Unauthorized,
    )]
    pub collection: Account<'info, Collection>,
    pub authority: Signer<'info>,
}

pub fn handler(ctx: Context<TogglePause>) -> Result<()> {
    let collection = &mut ctx.accounts.collection;
    collection.paused = !collection.paused;
    Ok(())
}
