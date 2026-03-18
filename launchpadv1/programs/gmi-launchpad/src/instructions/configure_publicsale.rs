use anchor_lang::prelude::*;
use crate::state::Collection;
use crate::errors::LaunchpadError;

#[derive(Accounts)]
pub struct ConfigurePublicsale<'info> {
    #[account(
        mut,
        seeds = [Collection::SEED_PREFIX, collection.collection_id.as_ref()],
        bump = collection.bump,
        constraint = authority.key() == collection.authority @ LaunchpadError::Unauthorized,
    )]
    pub collection: Account<'info, Collection>,
    pub authority: Signer<'info>,
}

pub fn handler(
    ctx: Context<ConfigurePublicsale>, 
    mint_price: Option<u64>, 
    max_user_mint_amount: u64, 
    max_tx_mint_amount: u64,
    publicsale_active: bool
) -> Result<()> {
    let collection = &mut ctx.accounts.collection;
    if let Some(price) = mint_price {
        collection.mint_price = price;
    }
    collection.max_user_mint_amount = max_user_mint_amount;
    collection.max_tx_mint_amount = max_tx_mint_amount;
    collection.publicsale_active = publicsale_active;
    Ok(())
}
