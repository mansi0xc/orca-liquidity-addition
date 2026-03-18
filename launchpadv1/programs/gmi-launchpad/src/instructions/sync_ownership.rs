use anchor_lang::prelude::*;
use crate::state::{Collection, TokenRecord};
use anchor_spl::token::TokenAccount;
use crate::errors::LaunchpadError;
use crate::events::OwnershipSynced;
use operator_registry::state::OperatorRegistryState;
use crate::utils;

#[derive(Accounts)]
pub struct SyncOwnership<'info> {
    #[account(mut)]
    pub collection: Account<'info, Collection>,

    #[account(
        mut,
        seeds = [
            TokenRecord::SEED_PREFIX,
            collection.key().as_ref(),
            nft_token_account.mint.as_ref(),
        ],
        bump = token_record.bump,
    )]
    pub token_record: Account<'info, TokenRecord>,

    #[account(
        constraint = nft_token_account.owner == current_owner.key() @ LaunchpadError::NotTokenOwner,
        constraint = nft_token_account.amount == 1 @ LaunchpadError::TokenAccountEmpty,
    )]
    pub nft_token_account: Account<'info, TokenAccount>,

    #[account(mut)]
    pub current_owner: Signer<'info>,

    #[account(
        constraint = collection.has_operator_filter @ LaunchpadError::OperatorNotWhitelisted
    )]
    pub operator_registry_state: Account<'info, OperatorRegistryState>,

    #[account(
        mut,
        constraint = fund_receiver.key() == operator_registry_state.fund_receiver @ LaunchpadError::Unauthorized,
    )]
    /// CHECK: Handled securely
    pub fund_receiver: SystemAccount<'info>,

    pub system_program: Program<'info, System>,
}

pub fn handler(ctx: Context<SyncOwnership>) -> Result<()> {
    let token_record = &mut ctx.accounts.token_record;
    
    // Check if it actually needs syncing
    require!(
        ctx.accounts.nft_token_account.owner != token_record.owner,
        LaunchpadError::AlreadySettled
    );

    // EXACT MATH PROTOCOL FEE PENALTY
    // The exact fee is derived mathematically from the moment the token was minted, NOT the current registry rate.
    // This provides a 100% deterministic economic recovery profile.
    let required_fee = (token_record.original_mint_price as u128)
        .checked_mul(token_record.protocol_fee_bps as u128)
        .ok_or(LaunchpadError::ArithmeticOverflow)?
        .checked_div(10000)
        .ok_or(LaunchpadError::PriceTruncationError)? as u64;

    if required_fee > 0 {
        utils::transfer_sol(
            &ctx.accounts.current_owner.to_account_info(),
            &ctx.accounts.fund_receiver.to_account_info(),
            &ctx.accounts.system_program.to_account_info(),
            required_fee,
        )?;
    }

    // Recover utility
    token_record.owner = ctx.accounts.current_owner.key();

    emit!(OwnershipSynced {
        collection: ctx.accounts.collection.key(),
        mint: ctx.accounts.nft_token_account.mint,
        new_owner: ctx.accounts.current_owner.key(),
        fee_paid: required_fee,
    });

    Ok(())
}
