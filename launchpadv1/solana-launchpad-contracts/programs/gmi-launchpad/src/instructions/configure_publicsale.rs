use anchor_lang::prelude::*;
use crate::state::Collection;
use crate::errors::LaunchpadError;
use crate::events::{MintPriceChanged, MaxUserMintAmountChanged, MaxTxMintAmountChanged, PublicsaleToggled};

#[derive(Accounts)]
pub struct ConfigurePublicsale<'info> {
    #[account(
        mut,
        seeds = [Collection::SEED_PREFIX, collection.collection_id.as_ref()],
        bump = collection.bump,
        has_one = authority @ LaunchpadError::Unauthorized,
    )]
    pub collection: Account<'info, Collection>,

    pub authority: Signer<'info>,
}

pub fn handler(
    ctx: Context<ConfigurePublicsale>,
    mint_price: Option<u64>,
    max_user_mint_amount: u64,
    max_tx_mint_amount: u64,
    publicsale_active: bool,
) -> Result<()> {
    let collection = &mut ctx.accounts.collection;
    let collection_key = collection.key();

    // Update mint price if provided (Standard and C variants allow price change)
    if let Some(price) = mint_price {
        collection.mint_price = price;
        emit!(MintPriceChanged {
            collection: collection_key,
            new_mint_price: price,
        });
    }

    collection.max_user_mint_amount = max_user_mint_amount;
    emit!(MaxUserMintAmountChanged {
        collection: collection_key,
        new_max_user_mint_amount: max_user_mint_amount,
    });

    collection.max_tx_mint_amount = max_tx_mint_amount;
    emit!(MaxTxMintAmountChanged {
        collection: collection_key,
        new_max_tx_mint_amount: max_tx_mint_amount,
    });

    collection.publicsale_active = publicsale_active;
    emit!(PublicsaleToggled {
        collection: collection_key,
        publicsale_active,
    });

    Ok(())
}
