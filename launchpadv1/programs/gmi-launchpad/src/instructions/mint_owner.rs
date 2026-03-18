use anchor_lang::prelude::*;
use anchor_spl::token::{self, Mint, Token, TokenAccount, MintTo};
use crate::state::{Collection, TokenRecord};
use crate::errors::LaunchpadError;
use crate::events::Minted;

#[derive(Accounts)]
pub struct MintOwner<'info> {
    #[account(
        mut,
        seeds = [Collection::SEED_PREFIX, collection.collection_id.as_ref()],
        bump = collection.bump,
        constraint = authority.key() == collection.authority @ LaunchpadError::Unauthorized,
    )]
    pub collection: Account<'info, Collection>,

    #[account(
        init,
        payer = authority,
        space = TokenRecord::SIZE,
        seeds = [
            TokenRecord::SEED_PREFIX,
            collection.key().as_ref(),
            nft_mint.key().as_ref(),
        ],
        bump,
    )]
    pub token_record: Account<'info, TokenRecord>,

    #[account(
        init,
        payer = authority,
        mint::decimals = 0,
        mint::authority = collection,
    )]
    pub nft_mint: Account<'info, Mint>,

    #[account(
        init,
        payer = authority,
        token::mint = nft_mint,
        token::authority = authority,
    )]
    pub nft_token_account: Account<'info, TokenAccount>,

    #[account(mut)]
    pub authority: Signer<'info>,

    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
    pub rent: Sysvar<'info, Rent>,
}

pub fn handler(ctx: Context<MintOwner>) -> Result<()> {
    let collection = &mut ctx.accounts.collection;
    let actual_quantity: u64 = 1;

    require!(
        collection.minted_amount.checked_add(actual_quantity).unwrap() <= collection.max_mint_supply,
        LaunchpadError::MaxSupply
    );

    // Free mints count towards reserved_nfts for Refundable variants
    if collection.collection_type != crate::state::CollectionType::Standard {
        collection.reserved_mints = collection.reserved_mints.checked_add(actual_quantity).unwrap();
        require!(
            collection.reserved_mints <= collection.reserved_nfts,
            LaunchpadError::ReservedNftsMinted
        );
    }

    collection.minted_amount = collection.minted_amount.checked_add(actual_quantity).unwrap();
    collection.total_mints = collection.total_mints.checked_add(1).unwrap();
    let target_token_index = collection.total_mints;

    let collection_id = collection.collection_id;
    let bump = collection.bump;
    let collection_info = collection.to_account_info();

    let signer_seeds: &[&[&[u8]]] = &[&[
        Collection::SEED_PREFIX,
        collection_id.as_ref(),
        &[bump],
    ]];

    let cpi_ctx = CpiContext::new_with_signer(
        ctx.accounts.token_program.to_account_info(),
        MintTo {
            mint: ctx.accounts.nft_mint.to_account_info(),
            to: ctx.accounts.nft_token_account.to_account_info(),
            authority: collection_info,
        },
        signer_seeds,
    );
    token::mint_to(cpi_ctx, 1)?;

    let token_record = &mut ctx.accounts.token_record;
    token_record.collection = collection.key();
    token_record.mint = ctx.accounts.nft_mint.key();
    token_record.token_index = target_token_index;
    token_record.refund_price = 0; // Owner free mints have 0 refund
    token_record.is_owner_mint = true;
    token_record.owner = ctx.accounts.authority.key();
    token_record.transfer_count = 0;
    token_record.original_mint_price = 0;
    token_record.protocol_fee_bps = 0;
    token_record.bump = ctx.bumps.token_record;

    emit!(Minted {
        collection: collection.key(),
        user: ctx.accounts.authority.key(),
        token_index: target_token_index,
        is_remint: false,
    });

    Ok(())
}
