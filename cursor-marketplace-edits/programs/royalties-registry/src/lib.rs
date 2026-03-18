use anchor_lang::prelude::*;

pub mod errors;
pub mod events;
pub mod instructions;
pub mod state;

use instructions::*;

declare_id!("CdVAwwosJmSWZD4Yg4j6JW9Xm6YLYFLCytjkqBZwF8AQ");

#[program]
pub mod royalties_registry {
    use super::*;

    pub fn initialize(ctx: Context<InitializeRegistry>) -> Result<()> {
        instructions::initialize::handler(ctx)
    }

    pub fn set_royalties_by_collection(
        ctx: Context<SetRoyaltiesByCollection>,
        args: SetRoyaltiesByCollectionArgs,
    ) -> Result<()> {
        instructions::set_royalties::handler_set_royalties_by_collection(ctx, args)
    }

    pub fn set_owner_royalties_by_token(
        ctx: Context<SetOwnerRoyaltiesByToken>,
        args: SetOwnerRoyaltiesByTokenArgs,
    ) -> Result<()> {
        instructions::set_royalties::handler_set_owner_royalties_by_token(ctx, args)
    }

    pub fn set_creator_royalties_by_token(
        ctx: Context<SetCreatorRoyaltiesByToken>,
        args: SetCreatorRoyaltiesByTokenArgs,
    ) -> Result<()> {
        instructions::set_royalties::handler_set_creator_royalties_by_token(ctx, args)
    }

    pub fn set_provider_by_collection(
        ctx: Context<SetProviderByCollection>,
        provider_program: Pubkey,
    ) -> Result<()> {
        instructions::admin::handler_set_provider_by_collection(ctx, provider_program)
    }

    pub fn transfer_registry_ownership(
        ctx: Context<TransferRegistryOwnership>,
        new_owner: Pubkey,
    ) -> Result<()> {
        instructions::admin::handler_transfer_registry_ownership(ctx, new_owner)
    }
}
