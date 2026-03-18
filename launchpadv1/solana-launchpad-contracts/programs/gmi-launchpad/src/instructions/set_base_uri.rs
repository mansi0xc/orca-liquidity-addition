use anchor_lang::prelude::*;
use crate::state::Collection;
use crate::errors::LaunchpadError;
use crate::events::BaseUriSet;

#[derive(Accounts)]
pub struct SetBaseUri<'info> {
    #[account(
        mut,
        seeds = [Collection::SEED_PREFIX, collection.collection_id.as_ref()],
        bump = collection.bump,
        has_one = authority @ LaunchpadError::Unauthorized,
    )]
    pub collection: Account<'info, Collection>,

    pub authority: Signer<'info>,
}

pub fn handler(ctx: Context<SetBaseUri>, uri: String) -> Result<()> {
    require!(uri.len() <= 200, LaunchpadError::BaseUriTooLong);

    ctx.accounts.collection.base_uri = uri.clone();

    emit!(BaseUriSet {
        collection: ctx.accounts.collection.key(),
        uri,
    });

    Ok(())
}
