use anchor_lang::prelude::*;
use anchor_spl::token::{self, Mint, Token, TokenAccount, MintTo};
use crate::state::{Collection, CollectionType, MintCounter, TokenRecord, WhitelistEntry};
use crate::errors::LaunchpadError;
use crate::events::PresaleMinted;
use crate::utils;
use operator_registry::state::OperatorRegistryState;

#[derive(Accounts)]
pub struct MintPresale<'info> {
    #[account(
        mut,
        seeds = [Collection::SEED_PREFIX, collection.collection_id.as_ref()],
        bump = collection.bump,
    )]
    pub collection: Box<Account<'info, Collection>>,

    #[account(
        seeds = [
            WhitelistEntry::SEED_PREFIX,
            collection.key().as_ref(),
            minter.key().as_ref(),
        ],
        bump = whitelist_entry.bump,
        constraint = whitelist_entry.collection == collection.key() @ LaunchpadError::NotWhitelisted,
        constraint = whitelist_entry.mint_limit > 0 @ LaunchpadError::NotWhitelisted,
    )]
    pub whitelist_entry: Box<Account<'info, WhitelistEntry>>,

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
    pub mint_counter: Box<Account<'info, MintCounter>>,

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

    #[account(
        init,
        payer = minter,
        mint::decimals = 0,
        mint::authority = collection,
    )]
    pub nft_mint: Account<'info, Mint>,

    #[account(
        init,
        payer = minter,
        token::mint = nft_mint,
        token::authority = minter,
    )]
    pub nft_token_account: Account<'info, TokenAccount>,

    #[account(
        mut,
        seeds = [Collection::VAULT_SEED_PREFIX, collection.key().as_ref()],
        bump = collection.vault_bump,
    )]
    pub vault: SystemAccount<'info>,

    #[account(
        mut,
        constraint = owner_account.key() == collection.authority @ LaunchpadError::Unauthorized,
    )]
    pub owner_account: SystemAccount<'info>,

    /// CHECK: Dynamically parsed via Account::try_from
    pub operator_registry_state: Option<AccountInfo<'info>>,
    
    #[account(mut)]
    pub fund_receiver: Option<SystemAccount<'info>>,

    #[account(mut)]
    pub minter: Signer<'info>,

    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
    pub rent: Sysvar<'info, Rent>,
}

pub fn handler(ctx: Context<MintPresale>) -> Result<()> {
    let collection = &mut ctx.accounts.collection;
    let actual_quantity: u64 = 1;
    
    if collection.has_operator_filter {
        let registry_info = ctx.accounts.operator_registry_state.as_ref().ok_or(LaunchpadError::Unauthorized)?;
        let data = registry_info.try_borrow_data()?;
        let mut data_slice: &[u8] = &data;
        let registry = OperatorRegistryState::try_deserialize(&mut data_slice)?;
        
        require!(registry_info.key() == collection.operator_registry, LaunchpadError::Unauthorized);
        
        if let Some(fr) = ctx.accounts.fund_receiver.as_ref() {
            require!(fr.key() == registry.fund_receiver, LaunchpadError::Unauthorized);
        } else {
            return err!(LaunchpadError::Unauthorized);
        }
    }

    require!(!collection.paused, LaunchpadError::Paused);
    require!(collection.presale_active, LaunchpadError::PresaleNotActive);

    // Global Cooldown Limits
    let current_slot = Clock::get()?.slot;
    require!(
        current_slot.saturating_sub(collection.global_last_mint_slot) >= collection.global_min_slot_cooldown,
        LaunchpadError::GlobalRateLimitExceeded
    );
    collection.global_last_mint_slot = current_slot;

    // User Cooldown & Supply Validations
    let mint_counter = &mut ctx.accounts.mint_counter;
    if mint_counter.collection == Pubkey::default() {
        mint_counter.collection = collection.key();
        mint_counter.user = ctx.accounts.minter.key();
        mint_counter.bump = ctx.bumps.mint_counter;
        mint_counter.last_mint_slot = 0;
        mint_counter.number_minted = 0;
        mint_counter.presale_number_minted = 0;
    }

    require!(
        current_slot.saturating_sub(mint_counter.last_mint_slot) >= collection.min_slot_cooldown,
        LaunchpadError::UserRateLimitExceeded
    );
    mint_counter.last_mint_slot = current_slot;

    mint_counter.presale_number_minted = mint_counter.presale_number_minted.checked_add(actual_quantity).unwrap();
    require!(
        mint_counter.presale_number_minted <= ctx.accounts.whitelist_entry.mint_limit,
        LaunchpadError::MaxUserAmount
    );
    
    let is_free_presale = collection.presale_mint_price == 0;
    if is_free_presale {
        collection.reserved_mints = collection.reserved_mints.checked_add(actual_quantity).unwrap();
        require!(
            collection.reserved_mints <= collection.reserved_nfts,
            LaunchpadError::ReservedNftsMinted
        );
    }

    collection.minted_amount = collection.minted_amount.checked_add(actual_quantity).unwrap();
    require!(
        collection.minted_amount <= collection.max_mint_supply,
        LaunchpadError::MaxSupply
    );

    collection.total_mints = collection.total_mints.checked_add(1).unwrap();
    let target_token_index = collection.total_mints;

    let applied_price = collection.presale_mint_price;
    let original_price = applied_price;

    // Registry Protocol Fee
    let mut protocol_fee_bps = 0;
    let mut protocol_fee = 0;
    
    if applied_price > 0 && collection.has_operator_filter {
        let registry_info = ctx.accounts.operator_registry_state.as_ref().unwrap();
        let data = registry_info.try_borrow_data()?;
        let mut data_slice: &[u8] = &data;
        let registry = OperatorRegistryState::try_deserialize(&mut data_slice)?;
        
        if matches!(collection.collection_type, CollectionType::Refundable100 | CollectionType::Refundable80) {
            protocol_fee_bps = registry.share_percentage_bps;
            protocol_fee = utils::calculate_protocol_fee(applied_price, protocol_fee_bps)?;
            
            if protocol_fee > 0 {
                let fr = ctx.accounts.fund_receiver.as_ref().unwrap();
                utils::transfer_sol(
                    &ctx.accounts.minter.to_account_info(),
                    &fr.to_account_info(),
                    &ctx.accounts.system_program.to_account_info(),
                    protocol_fee,
                )?;
            }
        }
    }

    let net_price = applied_price.checked_sub(protocol_fee).unwrap();
    let (vault_cut, owner_cut) = utils::calculate_vault_and_owner_cut(collection.collection_type, net_price)?;

    if owner_cut > 0 {
        utils::transfer_sol(
            &ctx.accounts.minter.to_account_info(),
            &ctx.accounts.owner_account.to_account_info(),
            &ctx.accounts.system_program.to_account_info(),
            owner_cut,
        )?;
    }
    if vault_cut > 0 {
        utils::transfer_sol(
            &ctx.accounts.minter.to_account_info(),
            &ctx.accounts.vault.to_account_info(),
            &ctx.accounts.system_program.to_account_info(),
            vault_cut,
        )?;
    }

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
    
    let (refund_amount, _) = utils::calculate_vault_and_owner_cut(collection.collection_type, applied_price)?;
    token_record.refund_price = refund_amount;
    
    token_record.is_owner_mint = false;
    token_record.owner = ctx.accounts.minter.key();
    token_record.transfer_count = 0;
    token_record.original_mint_price = original_price; // Note: original_price implies standard price, but for presale we store presale price
    token_record.protocol_fee_bps = protocol_fee_bps;
    token_record.bump = ctx.bumps.token_record;

    emit!(PresaleMinted {
        collection: collection.key(),
        user: ctx.accounts.minter.key(),
        token_index: target_token_index,
        is_remint: false,
    });

    Ok(())
}
