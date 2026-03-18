use anchor_lang::prelude::*;
use crate::state::{Collection, CollectionType};
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

    /// CHECK: Vault PDA for holding SOL (only initialized for refundable types).
    /// Seeds: ["vault", collection]
    #[account(
        mut,
        seeds = [Collection::VAULT_SEED_PREFIX, collection.key().as_ref()],
        bump,
    )]
    pub vault: SystemAccount<'info>,

    #[account(mut)]
    pub authority: Signer<'info>,

    pub system_program: Program<'info, System>,
}

pub fn handler(
    ctx: Context<InitializeCollection>,
    params: InitializeCollectionParams,
) -> Result<()> {
    // Validate inputs
    require!(params.max_mint_supply > 0, LaunchpadError::ZeroQuantity);
    require!(params.name.len() <= 32, LaunchpadError::NameTooLong);
    require!(params.symbol.len() <= 10, LaunchpadError::SymbolTooLong);
    require!(params.base_uri.len() <= 200, LaunchpadError::BaseUriTooLong);

    // Calculate reserved NFTs for refundable variants
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

    // Supply
    collection.max_mint_supply = params.max_mint_supply;
    collection.minted_amount = 0;
    collection.total_mints = 0;
    collection.refund_counter = 0;

    // Pricing
    collection.mint_price = params.mint_price;
    collection.presale_mint_price = params.presale_mint_price;

    // Limits
    collection.max_user_mint_amount = params.max_user_mint_amount;
    collection.max_tx_mint_amount = params.max_tx_mint_amount;
    collection.presale_max_user_mint_amount = params.presale_max_user_mint_amount;
    collection.presale_max_tx_mint_amount = params.presale_max_tx_mint_amount;

    // Status
    collection.presale_active = false;
    collection.publicsale_active = false;
    collection.paused = false;

    // Reserved
    collection.reserved_nfts = reserved_nfts;
    collection.reserved_mints = 0;

    // Metadata
    collection.name = params.name.clone();
    collection.symbol = params.symbol.clone();
    collection.base_uri = params.base_uri;

    // PDA bumps
    collection.bump = ctx.bumps.collection;
    collection.vault_bump = ctx.bumps.vault;

    emit!(CollectionInitialized {
        collection: collection.key(),
        authority: ctx.accounts.authority.key(),
        name: params.name,
        symbol: params.symbol,
        collection_type: params.collection_type as u8,
    });

    Ok(())
}
