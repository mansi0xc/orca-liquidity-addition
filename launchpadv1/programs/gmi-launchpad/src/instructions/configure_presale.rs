use anchor_lang::prelude::*;
use crate::state::Collection;
use crate::errors::LaunchpadError;

#[derive(Accounts)]
pub struct ConfigurePresale<'info> {
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
    ctx: Context<ConfigurePresale>, 
    presale_mint_price: Option<u64>, 
    presale_max_user_mint_amount: u64, 
    presale_max_tx_mint_amount: u64,
    presale_active: bool
) -> Result<()> {
    let collection = &mut ctx.accounts.collection;
    if let Some(price) = presale_mint_price {
        collection.presale_mint_price = price;
    }
    collection.presale_max_user_mint_amount = presale_max_user_mint_amount;
    collection.presale_max_tx_mint_amount = presale_max_tx_mint_amount;
    collection.presale_active = presale_active;
    Ok(())
}
