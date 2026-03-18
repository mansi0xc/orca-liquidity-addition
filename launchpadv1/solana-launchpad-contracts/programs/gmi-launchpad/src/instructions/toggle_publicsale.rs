use anchor_lang::prelude::*;
use crate::state::Collection;
use crate::errors::LaunchpadError;
use crate::events::PublicsaleToggled;

#[derive(Accounts)]
pub struct TogglePublicsale<'info> {
    #[account(
        mut,
        seeds = [Collection::SEED_PREFIX, collection.collection_id.as_ref()],
        bump = collection.bump,
        has_one = authority @ LaunchpadError::Unauthorized,
    )]
    pub collection: Account<'info, Collection>,

    pub authority: Signer<'info>,
}

pub fn handler(ctx: Context<TogglePublicsale>) -> Result<()> {
    let collection = &mut ctx.accounts.collection;
    collection.publicsale_active = !collection.publicsale_active;

    // NOTE: Emits NEW state — fixes EVM bug IB15
    emit!(PublicsaleToggled {
        collection: collection.key(),
        publicsale_active: collection.publicsale_active,
    });

    Ok(())
}
