use anchor_lang::prelude::*;
use crate::state::{Collection, CollectionType};
use crate::errors::LaunchpadError;
use crate::utils;

#[derive(Accounts)]
pub struct WithdrawVault<'info> {
    #[account(
        mut,
        seeds = [Collection::SEED_PREFIX, collection.collection_id.as_ref()],
        bump = collection.bump,
        constraint = collection.authority == authority.key() @ LaunchpadError::Unauthorized,
    )]
    pub collection: Account<'info, Collection>,

    #[account(
        mut,
        seeds = [Collection::VAULT_SEED_PREFIX, collection.key().as_ref()],
        bump = collection.vault_bump,
    )]
    /// CHECK: Vault PDA bounded by logic
    pub vault: SystemAccount<'info>,

    #[account(mut)]
    pub authority: Signer<'info>,
}

pub fn handler(ctx: Context<WithdrawVault>) -> Result<()> {
    let collection = &ctx.accounts.collection;

    // F8: Withdraw is strictly disallowed for refundable variants while active.
    // In Standard, we can withdraw stuck funds if any.
    // In Refundable, withdraw is physically locked to secure refunds until epoch expiry (if applicable in v2).
    require!(
        collection.collection_type == CollectionType::Standard,
        LaunchpadError::Unauthorized // Or a better error, mapped to EVM F8 constraint
    );

    let vault_balance = ctx.accounts.vault.lamports();
    if vault_balance > 0 {
        utils::transfer_sol_from_vault(
            &ctx.accounts.vault.to_account_info(),
            &ctx.accounts.authority.to_account_info(),
            vault_balance,
        )?;
    }

    Ok(())
}
