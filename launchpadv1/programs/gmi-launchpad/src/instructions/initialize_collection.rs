use anchor_lang::prelude::*;
use crate::state::{Collection, CollectionType, RefundBitmap};
use crate::errors::LaunchpadError;
use crate::events::CollectionInitialized;
use crate::utils;

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct InitializeCollectionParams {
    pub collection_id: [u8; 32],
    pub collection_type: CollectionType,
    pub has_operator_filter: bool,
    pub operator_registry: Pubkey,
    pub max_mint_supply: u64,
    pub mint_price: u64,
    pub presale_mint_price: u64,
    pub max_user_mint_amount: u64,
    pub max_tx_mint_amount: u64,
    pub presale_max_user_mint_amount: u64,
    pub presale_max_tx_mint_amount: u64,
    pub name: String,
    pub symbol: String,
    pub base_uri: String,
}

#[derive(Accounts)]
#[instruction(params: InitializeCollectionParams)]
pub struct InitializeCollection<'info> {
    #[account(
        init,
        payer = authority,
        space = Collection::space(
            params.name.len(),
            params.symbol.len(),
            params.base_uri.len(),
        ),
        seeds = [Collection::SEED_PREFIX, params.collection_id.as_ref()],
        bump,
    )]
    pub collection: Account<'info, Collection>,

    #[account(
        mut,
        seeds = [Collection::VAULT_SEED_PREFIX, collection.key().as_ref()],
        bump,
    )]
    /// CHECK: Vault PDA funded with base rent to prevent exhaustion DoS
    pub vault: SystemAccount<'info>,
    
    #[account(
        init,
        payer = authority,
        space = RefundBitmap::SIZE,
        seeds = [RefundBitmap::SEED_PREFIX, collection.key().as_ref()],
        bump,
    )]
    pub refund_bitmap: Box<Account<'info, RefundBitmap>>,

    #[account(mut)]
    pub authority: Signer<'info>,

    pub system_program: Program<'info, System>,
    pub rent: Sysvar<'info, Rent>,
}

pub fn handler(
    ctx: Context<InitializeCollection>,
    params: InitializeCollectionParams,
) -> Result<()> {
    require!(params.max_mint_supply > 0 && params.max_mint_supply <= 10000, LaunchpadError::ZeroQuantity);
    require!(params.name.len() <= 32, LaunchpadError::NameTooLong);
    require!(params.symbol.len() <= 10, LaunchpadError::SymbolTooLong);
    require!(params.base_uri.len() <= 200, LaunchpadError::BaseUriTooLong);

    let reserved_nfts = match params.collection_type {
        CollectionType::Refundable100 | CollectionType::Refundable80 => {
            utils::calculate_reserved_nfts(params.max_mint_supply)?
        }
        CollectionType::Standard => 0,
    };

    let collection = &mut ctx.accounts.collection;
    collection.authority = ctx.accounts.authority.key();
    collection.collection_id = params.collection_id;
    collection.collection_type = params.collection_type;
    collection.has_operator_filter = params.has_operator_filter;
    collection.operator_registry = params.operator_registry;

    collection.max_mint_supply = params.max_mint_supply;
    collection.minted_amount = 0;
    collection.total_mints = 0;
    collection.refund_counter = 0;

    collection.mint_price = params.mint_price;
    collection.presale_mint_price = params.presale_mint_price;

    collection.max_user_mint_amount = params.max_user_mint_amount;
    collection.max_tx_mint_amount = params.max_tx_mint_amount; 
    collection.presale_max_user_mint_amount = params.presale_max_user_mint_amount;
    collection.presale_max_tx_mint_amount = params.presale_max_tx_mint_amount; 

    collection.presale_active = false;
    collection.publicsale_active = false;
    collection.paused = false;

    collection.reserved_nfts = reserved_nfts;
    collection.reserved_mints = 0;

    collection.name = params.name.clone();
    collection.symbol = params.symbol.clone();
    collection.base_uri = params.base_uri;

    collection.available_remints = 0;
    collection.global_last_mint_slot = 0;
    collection.min_slot_cooldown = 1; // Default deterministic cooldowns instead of arbitrary limits
    collection.global_min_slot_cooldown = 0;

    collection.bump = ctx.bumps.collection;
    collection.vault_bump = ctx.bumps.vault;
    
    let refund_bitmap = &mut ctx.accounts.refund_bitmap;
    refund_bitmap.collection = collection.key();
    refund_bitmap.search_cursor = 0;
    refund_bitmap.bitmap = [0; 1250];
    refund_bitmap.bump = ctx.bumps.refund_bitmap;

    // Prefund Vault PDA to prevent rent exhaustion DoS
    let rent_minimum = ctx.accounts.rent.minimum_balance(0);
    utils::transfer_sol(
        &ctx.accounts.authority.to_account_info(),
        &ctx.accounts.vault.to_account_info(),
        &ctx.accounts.system_program.to_account_info(),
        rent_minimum,
    )?;

    emit!(CollectionInitialized {
        collection: collection.key(),
        authority: ctx.accounts.authority.key(),
        name: params.name,
        symbol: params.symbol,
        collection_type: params.collection_type as u8,
    });

    Ok(())
}
