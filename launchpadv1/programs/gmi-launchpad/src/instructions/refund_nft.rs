use anchor_lang::prelude::*;
use anchor_spl::token::{self, Burn, CloseAccount, Mint, Token, TokenAccount};
use crate::state::{Collection, CollectionType, TokenRecord, RefundBitmap};
use crate::errors::LaunchpadError;
use crate::events::Refunded;
use crate::utils;

#[derive(Accounts)]
pub struct RefundNft<'info> {
    #[account(
        mut,
        seeds = [Collection::SEED_PREFIX, collection.collection_id.as_ref()],
        bump = collection.bump,
        constraint = collection.collection_type != CollectionType::Standard @ LaunchpadError::RefundNotSupported,
    )]
    pub collection: Box<Account<'info, Collection>>,

    #[account(
        mut,
        close = owner,
        seeds = [
            TokenRecord::SEED_PREFIX,
            collection.key().as_ref(),
            nft_mint.key().as_ref(),
        ],
        bump = token_record.bump,
        constraint = token_record.collection == collection.key() @ LaunchpadError::InvalidMint,
        constraint = !token_record.is_owner_mint @ LaunchpadError::OwnerMintNotRefundable,
        constraint = token_record.refund_price > 0 @ LaunchpadError::FreeNftNotRefundable,
    )]
    pub token_record: Box<Account<'info, TokenRecord>>,

    #[account(
        mut,
        seeds = [RefundBitmap::SEED_PREFIX, collection.key().as_ref()],
        bump = refund_bitmap.bump,
    )]
    pub refund_bitmap: Box<Account<'info, RefundBitmap>>,

    #[account(
        mut,
        constraint = nft_mint.supply == 1 @ LaunchpadError::InvalidMint,
    )]
    pub nft_mint: Account<'info, Mint>,

    #[account(
        mut,
        constraint = nft_token_account.mint == nft_mint.key() @ LaunchpadError::InvalidTokenAccount,
        constraint = nft_token_account.owner == owner.key() @ LaunchpadError::NotTokenOwner,
        constraint = nft_token_account.amount == 1 @ LaunchpadError::TokenAccountEmpty,
    )]
    pub nft_token_account: Account<'info, TokenAccount>,

    #[account(
        mut,
        seeds = [Collection::VAULT_SEED_PREFIX, collection.key().as_ref()],
        bump = collection.vault_bump,
    )]
    pub vault: SystemAccount<'info>,

    #[account(mut)]
    pub owner: Signer<'info>,

    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}

pub fn handler(ctx: Context<RefundNft>) -> Result<()> {
    // ENFORCE SETTLED OWNERSHIP INVARIANT
    require!(
        ctx.accounts.nft_token_account.owner == ctx.accounts.token_record.owner, 
        LaunchpadError::UnsettledState
    );
    // Double check that the signer is indeed the owner
    require!(
        ctx.accounts.owner.key() == ctx.accounts.token_record.owner, 
        LaunchpadError::Unauthorized
    );

    let refund_amount = ctx.accounts.token_record.refund_price;
    let token_index = ctx.accounts.token_record.token_index;
    let nft_mint_key = ctx.accounts.nft_mint.key();

    // Burn the NFT
    let cpi_ctx = CpiContext::new(
        ctx.accounts.token_program.to_account_info(),
        Burn {
            mint: ctx.accounts.nft_mint.to_account_info(),
            from: ctx.accounts.nft_token_account.to_account_info(),
            authority: ctx.accounts.owner.to_account_info(),
        },
    );
    token::burn(cpi_ctx, 1)?;

    let close_ctx = CpiContext::new(
        ctx.accounts.token_program.to_account_info(),
        CloseAccount {
            account: ctx.accounts.nft_token_account.to_account_info(),
            destination: ctx.accounts.owner.to_account_info(),
            authority: ctx.accounts.owner.to_account_info(),
        },
    );
    token::close_account(close_ctx)?;

    // Transfer refund from vault to owner
    // Vault rent is isolated, so vault dropping to 0 here is mathematically impossible
    utils::transfer_sol_from_vault(
        &ctx.accounts.vault.to_account_info(),
        &ctx.accounts.owner.to_account_info(),
        refund_amount,
    )?;

    let collection = &mut ctx.accounts.collection;
    collection.minted_amount = collection.minted_amount.checked_sub(1).ok_or(LaunchpadError::ArithmeticUnderflow)?;
    collection.refund_counter = collection.refund_counter.checked_add(1).ok_or(LaunchpadError::ArithmeticOverflow)?;
    
    if collection.collection_type == CollectionType::Refundable80 {
        collection.available_remints = collection.available_remints.checked_add(1).ok_or(LaunchpadError::ArithmeticOverflow)?;
        
        // Push exact identity to RefundBitmap
        let bitmap = &mut ctx.accounts.refund_bitmap;
        utils::set_refunded_bit(bitmap, token_index)?;
    }

    emit!(Refunded {
        collection: collection.key(),
        user: ctx.accounts.owner.key(),
        mint: nft_mint_key,
        token_index,
        refund_amount,
    });

    Ok(())
}
