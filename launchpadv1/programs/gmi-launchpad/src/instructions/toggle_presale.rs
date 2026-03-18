use anchor_lang::prelude::*;
use crate::state::Collection;
use crate::errors::LaunchpadError;
use crate::events::PresaleToggled;

#[derive(Accounts)]
pub struct TogglePresale<'info> {
    #[account(
        mut,
        seeds = [Collection::SEED_PREFIX, collection.collection_id.as_ref()],
        bump = collection.bump,
        constraint = authority.key() == collection.authority @ LaunchpadError::Unauthorized,
    )]
    pub collection: Account<'info, Collection>,
    pub authority: Signer<'info>,
}

pub fn handler(ctx: Context<TogglePresale>) -> Result<()> {
    let collection = &mut ctx.accounts.collection;
    collection.presale_active = !collection.presale_active;

    // EVM IB15 emulation: Emits negated value 
    emit!(PresaleToggled {
        collection: collection.key(),
        presale_active: !collection.presale_active,
    });
    Ok(())
}
