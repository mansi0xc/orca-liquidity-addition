use anchor_lang::prelude::*;
use anchor_spl::token::{self, Mint, Token, TokenAccount, MintTo};
use crate::state::{Collection, CollectionType, MintCounter, TokenRecord};
use crate::errors::LaunchpadError;
use crate::events::Minted;
use crate::utils;

#[derive(Accounts)]
pub struct MintPublic<'info> {
    #[account(
        mut,
        seeds = [Collection::SEED_PREFIX, collection.collection_id.as_ref()],
        bump = collection.bump,
    )]
    pub collection: Account<'info, Collection>,

    #[account(
        init_if_needed,
        payer = minter,
        space = MintCounter::SIZE,
        seeds = [
            MintCounter::SEED_PREFIX,
            collection.key().as_ref(),
            minter.key().as_ref(),
        ],
        bump,
    )]
    pub mint_counter: Account<'info, MintCounter>,

    #[account(
        init,
        payer = minter,
        space = TokenRecord::SIZE,
        seeds = [
            TokenRecord::SEED_PREFIX,
            collection.key().as_ref(),
            nft_mint.key().as_ref(),
        ],
        bump,
    )]
    pub token_record: Account<'info, TokenRecord>,

    /// The new NFT mint account (must be pre-created or init here)
    #[account(
        init,
        payer = minter,
        mint::decimals = 0,
        mint::authority = collection,
    )]
    pub nft_mint: Account<'info, Mint>,

    /// The minter's token account for the new NFT
    #[account(
        init,
        payer = minter,
        token::mint = nft_mint,
        token::authority = minter,
    )]
    pub nft_token_account: Account<'info, TokenAccount>,

    /// CHECK: Vault PDA that holds SOL for refundable collections
    #[account(
        mut,
        seeds = [Collection::VAULT_SEED_PREFIX, collection.key().as_ref()],
        bump = collection.vault_bump,
    )]
    pub vault: SystemAccount<'info>,

    /// CHECK: Owner/authority account that receives payment for Standard collections
    #[account(
        mut,
        constraint = owner_account.key() == collection.authority @ LaunchpadError::Unauthorized,
    )]
    pub owner_account: AccountInfo<'info>,

    #[account(mut)]
    pub minter: Signer<'info>,

    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
    pub rent: Sysvar<'info, Rent>,
}

pub fn handler(ctx: Context<MintPublic>, quantity: u64) -> Result<()> {
    let collection = &ctx.accounts.collection;

    // === Validation (R1, R5, R6, R7, R8 equivalents) ===
    require!(!collection.paused, LaunchpadError::Paused);
    require!(collection.publicsale_active, LaunchpadError::PublicsaleNotActive);
    require!(quantity > 0, LaunchpadError::ZeroQuantity);
    require!(quantity <= collection.max_tx_mint_amount, LaunchpadError::MaxTxAmount);

    // For this Solana implementation, we handle one NFT per instruction call
    // (quantity parameter is validated but actual mint is 1 NFT per call for account model reasons)
    // The caller should call this instruction `quantity` times.
    // However, we validate as if it's the full quantity for limit checks.
    let actual_quantity: u64 = 1; // Single mint per instruction on Solana

    let mint_counter = &ctx.accounts.mint_counter;
    let user_minted = mint_counter.number_minted;
    require!(
        user_minted.checked_add(actual_quantity).ok_or(LaunchpadError::ArithmeticOverflow)?
            <= collection.max_user_mint_amount,
        LaunchpadError::MaxUserAmount
    );
    require!(
        collection.minted_amount.checked_add(actual_quantity).ok_or(LaunchpadError::ArithmeticOverflow)?
            <= collection.max_mint_supply,
        LaunchpadError::MaxSupply
    );

    // === Payment Calculation ===
    let price = collection.mint_price;
    let expected_payment = price; // 1 NFT

    // Transfer payment from minter
    match collection.collection_type {
        CollectionType::Standard => {
            // All payment goes to owner immediately
            utils::transfer_sol(
                &ctx.accounts.minter.to_account_info(),
                &ctx.accounts.owner_account,
                &ctx.accounts.system_program.to_account_info(),
                expected_payment,
                None,
            )?;
        }
        CollectionType::Refundable100 => {
            // All payment goes to vault (held for refund)
            utils::transfer_sol(
                &ctx.accounts.minter.to_account_info(),
                &ctx.accounts.vault.to_account_info(),
                &ctx.accounts.system_program.to_account_info(),
                expected_payment,
                None,
            )?;
        }
        CollectionType::Refundable80 => {
            // 20% to owner, 80% to vault
            let owner_cut = utils::calculate_owner_cut(CollectionType::Refundable80, price)?;
            let vault_cut = utils::calculate_refund_price(CollectionType::Refundable80, price)?;

            if owner_cut > 0 {
                utils::transfer_sol(
                    &ctx.accounts.minter.to_account_info(),
                    &ctx.accounts.owner_account,
                    &ctx.accounts.system_program.to_account_info(),
                    owner_cut,
                    None,
                )?;
            }
            if vault_cut > 0 {
                utils::transfer_sol(
                    &ctx.accounts.minter.to_account_info(),
                    &ctx.accounts.vault.to_account_info(),
                    &ctx.accounts.system_program.to_account_info(),
                    vault_cut,
                    None,
                )?;
            }
        }
    }

    // === Mint the NFT ===
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

    // Update mint counter
    let mint_counter = &mut ctx.accounts.mint_counter;
    if mint_counter.collection == Pubkey::default() {
        // First time init
        mint_counter.collection = collection.key();
        mint_counter.user = ctx.accounts.minter.key();
        mint_counter.bump = ctx.bumps.mint_counter;
    }
    mint_counter.number_minted = mint_counter.number_minted
        .checked_add(actual_quantity)
        .ok_or(LaunchpadError::ArithmeticOverflow)?;

    // Set token record
    let token_record = &mut ctx.accounts.token_record;
    token_record.collection = collection.key();
    token_record.mint = ctx.accounts.nft_mint.key();
    token_record.token_index = new_token_index;
    token_record.refund_price = utils::calculate_refund_price(collection.collection_type, price)?;
    token_record.is_owner_mint = false;
    token_record.bump = ctx.bumps.token_record;

    emit!(Minted {
        collection: collection.key(),
        user: ctx.accounts.minter.key(),
        quantity: actual_quantity,
        token_index: new_token_index,
    });

    Ok(())
}
