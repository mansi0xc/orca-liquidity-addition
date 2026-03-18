use anchor_lang::prelude::*;

use crate::errors::RegistryError;
use crate::state::{RegistryConfig, RoyaltyProvider};

// ─── set_provider_by_collection ──────────────────────────────────────────────

#[derive(Accounts)]
pub struct SetProviderByCollection<'info> {
    #[account(
        seeds = [b"registry_config"],
        bump = registry_config.bump,
    )]
    pub registry_config: Account<'info, RegistryConfig>,

    #[account(
        constraint = authority.key() == registry_config.owner @ RegistryError::Unauthorized
    )]
    pub authority: Signer<'info>,

    #[account(
        init_if_needed,
        payer = payer,
        space = 8 + RoyaltyProvider::INIT_SPACE,
        seeds = [b"royalty_provider", collection_mint.key().as_ref()],
        bump,
    )]
    pub royalty_provider: Account<'info, RoyaltyProvider>,

    /// CHECK: Collection mint for PDA derivation.
    pub collection_mint: AccountInfo<'info>,

    #[account(mut)]
    pub payer: Signer<'info>,

    pub system_program: Program<'info, System>,
}

pub fn handler_set_provider_by_collection(
    ctx: Context<SetProviderByCollection>,
    provider_program: Pubkey,
) -> Result<()> {
    let rp = &mut ctx.accounts.royalty_provider;
    rp.provider_program = provider_program;
    rp.bump = ctx.bumps.royalty_provider;
    Ok(())
}

// ─── transfer_registry_ownership ─────────────────────────────────────────────

#[derive(Accounts)]
pub struct TransferRegistryOwnership<'info> {
    #[account(
        mut,
        seeds = [b"registry_config"],
        bump = registry_config.bump,
    )]
    pub registry_config: Account<'info, RegistryConfig>,

    #[account(
        constraint = owner.key() == registry_config.owner @ RegistryError::Unauthorized
    )]
    pub owner: Signer<'info>,
}

pub fn handler_transfer_registry_ownership(
    ctx: Context<TransferRegistryOwnership>,
    new_owner: Pubkey,
) -> Result<()> {
    ctx.accounts.registry_config.owner = new_owner;
    Ok(())
}
