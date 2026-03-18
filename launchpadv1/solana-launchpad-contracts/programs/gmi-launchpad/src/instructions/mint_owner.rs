use anchor_lang::prelude::*;
use anchor_spl::token::{self, Mint, Token, TokenAccount, MintTo};
use crate::state::{Collection, CollectionType, TokenRecord};
use crate::errors::LaunchpadError;
use crate::events::OwnerMinted;

#[derive(Accounts)]
pub struct MintOwner<'info> {
    #[account(
        mut,
        seeds = [Collection::SEED_PREFIX, collection.collection_id.as_ref()],
        bump = collection.bump,
        has_one = authority @ LaunchpadError::Unauthorized,
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

    /// The recipient's token account for the new NFT
    #[account(
        init,
        payer = authority,
        token::mint = nft_mint,
        token::authority = recipient,
    )]
    pub nft_token_account: Account<'info, TokenAccount>,

    /// CHECK: The recipient of the owner mint
    pub recipient: AccountInfo<'info>,

    #[account(mut)]
    pub authority: Signer<'info>,

    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
    pub rent: Sysvar<'info, Rent>,
}

pub fn handler(ctx: Context<MintOwner>, quantity: u64) -> Result<()> {
    let collection = &ctx.accounts.collection;

    // Validate (no pause check for owner mint, matching EVM behavior)
    require!(quantity > 0, LaunchpadError::ZeroQuantity);

    let actual_quantity: u64 = 1;

    require!(
        collection.minted_amount.checked_add(actual_quantity).ok_or(LaunchpadError::ArithmeticOverflow)?
            <= collection.max_mint_supply,
        LaunchpadError::MaxSupply
    );

    // Reserved check for refundable variants (EVM IB6, R15)
    match collection.collection_type {
        CollectionType::Refundable100 => {
            require!(
                collection.reserved_mints.checked_add(actual_quantity).ok_or(LaunchpadError::ArithmeticOverflow)?
                    <= collection.reserved_nfts,
                LaunchpadError::ReservedNftsMinted
            );
        }
        _ => {}
    }

    // === Mint NFT (no payment — owner mints are free, EVM IB4) ===
    let collection_id = ctx.accounts.collection.collection_id;
    let bump = ctx.accounts.collection.bump;
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
            authority: ctx.accounts.collection.to_account_info(),
        },
        signer_seeds,
    );
    token::mint_to(cpi_ctx, 1)?;

    // === Update State ===
    let collection = &mut ctx.accounts.collection;
    let new_token_index = collection.total_mints
        .checked_add(1)
        .ok_or(LaunchpadError::ArithmeticOverflow)?;

    collection.minted_amount = collection.minted_amount
        .checked_add(actual_quantity)
        .ok_or(LaunchpadError::ArithmeticOverflow)?;
    collection.total_mints = new_token_index;

    // Track reserved mints for applicable variants
    if collection.collection_type == CollectionType::Refundable100 {
        collection.reserved_mints = collection.reserved_mints
            .checked_add(actual_quantity)
            .ok_or(LaunchpadError::ArithmeticOverflow)?;
    }

    // Set token record — marked as owner mint (non-refundable, EVM IB5)
    let token_record = &mut ctx.accounts.token_record;
    token_record.collection = collection.key();
    token_record.mint = ctx.accounts.nft_mint.key();
    token_record.token_index = new_token_index;
    token_record.refund_price = 0; // Owner mints have zero refund price
    token_record.is_owner_mint = true; // Critical: blocks refund (EVM IB5)
    token_record.bump = ctx.bumps.token_record;

    emit!(OwnerMinted {
        collection: collection.key(),
        recipient: ctx.accounts.recipient.key(),
        quantity: actual_quantity,
        token_index: new_token_index,
    });

    Ok(())
}
