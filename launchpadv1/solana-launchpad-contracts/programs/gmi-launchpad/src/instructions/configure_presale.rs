use anchor_lang::prelude::*;
use crate::state::Collection;
use crate::errors::LaunchpadError;
use crate::events::{PresaleMintPriceChanged, PresaleMaxUserMintAmountChanged, PresaleMaxTxMintAmountChanged, PresaleToggled};

#[derive(Accounts)]
pub struct ConfigurePresale<'info> {
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
    ctx: Context<ConfigurePresale>,
    presale_mint_price: Option<u64>,
    presale_max_user_mint_amount: u64,
    presale_max_tx_mint_amount: u64,
    presale_active: bool,
) -> Result<()> {
    let collection = &mut ctx.accounts.collection;
    let collection_key = collection.key();

    if let Some(price) = presale_mint_price {
        collection.presale_mint_price = price;
        emit!(PresaleMintPriceChanged {
            collection: collection_key,
            new_presale_mint_price: price,
        });
    }

    collection.presale_max_user_mint_amount = presale_max_user_mint_amount;
    emit!(PresaleMaxUserMintAmountChanged {
        collection: collection_key,
        new_presale_max_user_mint_amount: presale_max_user_mint_amount,
    });

    collection.presale_max_tx_mint_amount = presale_max_tx_mint_amount;
    emit!(PresaleMaxTxMintAmountChanged {
        collection: collection_key,
        new_presale_max_tx_mint_amount: presale_max_tx_mint_amount,
    });

    collection.presale_active = presale_active;
    emit!(PresaleToggled {
        collection: collection_key,
        presale_active,
    });

    Ok(())
}
