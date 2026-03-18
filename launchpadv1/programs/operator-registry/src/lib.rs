use anchor_lang::prelude::*;

pub mod state;
pub mod errors;
pub mod events;
pub mod instructions;
use instructions::*;

declare_id!("93XKzAqt4RTyBZCJZc5yQmj3KY8B1N7uD85Q9vqDrdHv");

#[program]
pub mod operator_registry {
    use super::*;

    pub fn initialize_registry(
        ctx: Context<InitializeRegistry>,
        fund_receiver: Pubkey,
        share_percentage_bps: u64,
    ) -> Result<()> {
        initialize_registry::handler(ctx, fund_receiver, share_percentage_bps)
    }

    pub fn add_operator_whitelist(
        ctx: Context<AddOperatorWhitelist>,
    ) -> Result<()> {
        add_operator_whitelist::handler(ctx)
    }

    pub fn remove_operator_whitelist(
        ctx: Context<RemoveOperatorWhitelist>,
    ) -> Result<()> {
        remove_operator_whitelist::handler(ctx)
    }

    pub fn add_universal_operator(
        ctx: Context<AddUniversalOperator>,
    ) -> Result<()> {
        add_universal_operator::handler(ctx)
    }

    pub fn remove_universal_operator(
        ctx: Context<RemoveUniversalOperator>,
    ) -> Result<()> {
        remove_universal_operator::handler(ctx)
    }

    pub fn change_fund_receiver(
        ctx: Context<ChangeFundReceiver>,
        new_fund_receiver: Pubkey,
    ) -> Result<()> {
        change_fund_receiver::handler(ctx, new_fund_receiver)
    }

    pub fn change_share_percentage(
        ctx: Context<ChangeSharePercentage>,
        new_share_percentage_bps: u64,
    ) -> Result<()> {
        change_share_percentage::handler(ctx, new_share_percentage_bps)
    }

    pub fn toggle_registry_pause(
        ctx: Context<ToggleRegistryPause>,
    ) -> Result<()> {
        toggle_registry_pause::handler(ctx)
    }
}
