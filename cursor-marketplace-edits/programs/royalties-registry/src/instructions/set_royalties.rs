use anchor_lang::prelude::*;

use crate::errors::RegistryError;
use crate::events::{RoyaltiesSetForCollection, RoyaltiesSetForToken};
use crate::state::{
    RegistryConfig, CollectionRoyalties, OwnerTokenRoyalties, CreatorTokenRoyalties,
    types::{RoyaltyPart, MAX_ROYALTY_RECIPIENTS},
};

// ─── set_royalties_by_collection ─────────────────────────────────────────────

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct SetRoyaltiesByCollectionArgs {
    pub royalties: Vec<RoyaltyPart>,
}

#[derive(Accounts)]
#[instruction(args: SetRoyaltiesByCollectionArgs)]
pub struct SetRoyaltiesByCollection<'info> {
    #[account(
        seeds = [b"registry_config"],
        bump = registry_config.bump,
    )]
    pub registry_config: Account<'info, RegistryConfig>,

    /// Must be registry owner or collection authority.
    pub authority: Signer<'info>,

    #[account(
        init_if_needed,
        payer = payer,
        space = 8 + CollectionRoyalties::INIT_SPACE,
        seeds = [b"collection_royalties", collection_mint.key().as_ref()],
        bump,
    )]
    pub collection_royalties: Account<'info, CollectionRoyalties>,

    /// CHECK: The collection mint. Verified by seed derivation.
    pub collection_mint: AccountInfo<'info>,

    #[account(mut)]
    pub payer: Signer<'info>,

    pub system_program: Program<'info, System>,
}

pub fn handler_set_royalties_by_collection(
    ctx: Context<SetRoyaltiesByCollection>,
    args: SetRoyaltiesByCollectionArgs,
) -> Result<()> {
    // RULE OR-1: verify authority is registry owner or collection authority.
    // For now we check registry owner. Metaplex metadata check will be added
    // when integrating with mpl-token-metadata.
    require!(
        ctx.accounts.authority.key() == ctx.accounts.registry_config.owner,
        RegistryError::Unauthorized
    );

    require!(
        args.royalties.len() <= MAX_ROYALTY_RECIPIENTS,
        RegistryError::TooManyRecipients
    );

    let mut sum_bps: u16 = 0;
    let mut recipients = Vec::with_capacity(args.royalties.len());
    let mut bps_values = Vec::with_capacity(args.royalties.len());

    for royalty in &args.royalties {
        // RULE OR-4: no zero address recipients
        require!(
            royalty.account != Pubkey::default(),
            RegistryError::ZeroAddressRecipient
        );
        sum_bps = sum_bps.checked_add(royalty.value).ok_or(RegistryError::RoyaltiesTooHigh)?;
        recipients.push(royalty.account);
        bps_values.push(royalty.value);
    }

    require!(sum_bps <= 10000, RegistryError::RoyaltiesTooHigh);

    let cr = &mut ctx.accounts.collection_royalties;
    cr.initialized = true;
    cr.royalties = args.royalties;
    cr.bump = ctx.bumps.collection_royalties;

    emit!(RoyaltiesSetForCollection {
        collection_mint: ctx.accounts.collection_mint.key(),
        recipients,
        bps_values,
    });

    Ok(())
}

// ─── set_owner_royalties_by_token ────────────────────────────────────────────

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct SetOwnerRoyaltiesByTokenArgs {
    pub token_id: u64,
    pub royalties: Vec<RoyaltyPart>,
}

#[derive(Accounts)]
#[instruction(args: SetOwnerRoyaltiesByTokenArgs)]
pub struct SetOwnerRoyaltiesByToken<'info> {
    #[account(
        seeds = [b"registry_config"],
        bump = registry_config.bump,
    )]
    pub registry_config: Account<'info, RegistryConfig>,

    pub authority: Signer<'info>,

    #[account(
        init_if_needed,
        payer = payer,
        space = 8 + OwnerTokenRoyalties::INIT_SPACE,
        seeds = [b"owner_royalties", collection_mint.key().as_ref(), &args.token_id.to_le_bytes()],
        bump,
    )]
    pub owner_token_royalties: Account<'info, OwnerTokenRoyalties>,

    /// CHECK: Collection mint for PDA derivation.
    pub collection_mint: AccountInfo<'info>,

    #[account(mut)]
    pub payer: Signer<'info>,

    pub system_program: Program<'info, System>,
}

pub fn handler_set_owner_royalties_by_token(
    ctx: Context<SetOwnerRoyaltiesByToken>,
    args: SetOwnerRoyaltiesByTokenArgs,
) -> Result<()> {
    // RULE OR-2: verify authority
    require!(
        ctx.accounts.authority.key() == ctx.accounts.registry_config.owner,
        RegistryError::Unauthorized
    );

    require!(
        args.royalties.len() <= MAX_ROYALTY_RECIPIENTS,
        RegistryError::TooManyRecipients
    );

    let mut sum_bps: u16 = 0;
    let mut recipients = Vec::with_capacity(args.royalties.len());
    let mut bps_values = Vec::with_capacity(args.royalties.len());

    for royalty in &args.royalties {
        require!(
            royalty.account != Pubkey::default(),
            RegistryError::ZeroAddressRecipient
        );
        sum_bps = sum_bps.checked_add(royalty.value).ok_or(RegistryError::RoyaltiesTooHigh)?;
        recipients.push(royalty.account);
        bps_values.push(royalty.value);
    }

    require!(sum_bps <= 10000, RegistryError::RoyaltiesTooHigh);

    let otr = &mut ctx.accounts.owner_token_royalties;
    otr.initialized = true;
    otr.royalties = args.royalties;
    otr.bump = ctx.bumps.owner_token_royalties;

    emit!(RoyaltiesSetForToken {
        collection_mint: ctx.accounts.collection_mint.key(),
        token_id: args.token_id,
        recipients,
        bps_values,
        setter_type: 0, // OWNER
    });

    Ok(())
}

// ─── set_creator_royalties_by_token ──────────────────────────────────────────

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct SetCreatorRoyaltiesByTokenArgs {
    pub token_id: u64,
    pub royalties: Vec<RoyaltyPart>,
}

#[derive(Accounts)]
#[instruction(args: SetCreatorRoyaltiesByTokenArgs)]
pub struct SetCreatorRoyaltiesByToken<'info> {
    #[account(
        seeds = [b"registry_config"],
        bump = registry_config.bump,
    )]
    pub registry_config: Account<'info, RegistryConfig>,

    /// Must be registry owner or token creator.
    pub authority: Signer<'info>,

    #[account(
        init_if_needed,
        payer = payer,
        space = 8 + CreatorTokenRoyalties::INIT_SPACE,
        seeds = [b"creator_royalties", collection_mint.key().as_ref(), &args.token_id.to_le_bytes()],
        bump,
    )]
    pub creator_token_royalties: Account<'info, CreatorTokenRoyalties>,

    /// CHECK: Collection mint for PDA derivation.
    pub collection_mint: AccountInfo<'info>,

    #[account(mut)]
    pub payer: Signer<'info>,

    pub system_program: Program<'info, System>,
}

pub fn handler_set_creator_royalties_by_token(
    ctx: Context<SetCreatorRoyaltiesByToken>,
    args: SetCreatorRoyaltiesByTokenArgs,
) -> Result<()> {
    // RULE OR-3: verify authority is registry owner or token creator.
    // Metaplex metadata creator verification will be added during integration.
    require!(
        ctx.accounts.authority.key() == ctx.accounts.registry_config.owner,
        RegistryError::Unauthorized
    );

    require!(
        args.royalties.len() <= MAX_ROYALTY_RECIPIENTS,
        RegistryError::TooManyRecipients
    );

    let mut sum_bps: u16 = 0;
    let mut recipients = Vec::with_capacity(args.royalties.len());
    let mut bps_values = Vec::with_capacity(args.royalties.len());

    for royalty in &args.royalties {
        require!(
            royalty.account != Pubkey::default(),
            RegistryError::ZeroAddressRecipient
        );
        sum_bps = sum_bps.checked_add(royalty.value).ok_or(RegistryError::RoyaltiesTooHigh)?;
        recipients.push(royalty.account);
        bps_values.push(royalty.value);
    }

    require!(sum_bps <= 10000, RegistryError::RoyaltiesTooHigh);

    let ctr = &mut ctx.accounts.creator_token_royalties;
    ctr.initialized = true;
    ctr.royalties = args.royalties;
    ctr.bump = ctx.bumps.creator_token_royalties;

    emit!(RoyaltiesSetForToken {
        collection_mint: ctx.accounts.collection_mint.key(),
        token_id: args.token_id,
        recipients,
        bps_values,
        setter_type: 1, // CREATOR
    });

    Ok(())
}
