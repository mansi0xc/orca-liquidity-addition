use anchor_lang::prelude::*;

pub mod errors;
pub mod events;
pub mod instructions;
pub mod logic;
pub mod state;

use instructions::*;

declare_id!("CuUtAt1GpoDoDGnrQbjeZTH7qSPdVwahybi4KoucQ6zN");

#[program]
pub mod exchange {
    use super::*;

    pub fn initialize(ctx: Context<Initialize>, args: InitializeArgs) -> Result<()> {
        instructions::initialize::handler_initialize(ctx, args)
    }

    pub fn match_orders<'info>(
        ctx: Context<'_, '_, 'info, 'info, MatchOrders<'info>>,
        args: MatchOrdersArgs,
    ) -> Result<()> {
        instructions::match_orders::handler_match_orders(ctx, args)
    }

    pub fn cancel_order(ctx: Context<CancelOrder>, args: CancelOrderArgs) -> Result<()> {
        instructions::cancel_order::handler_cancel_order(ctx, args)
    }

    pub fn set_protocol_fee_bps(
        ctx: Context<SetProtocolFeeBps>,
        new_protocol_fee_bps: u16,
    ) -> Result<()> {
        instructions::admin::handler_set_protocol_fee_bps(ctx, new_protocol_fee_bps)
    }

    pub fn set_default_fee_receiver(
        ctx: Context<SetDefaultFeeReceiver>,
        new_default_fee_receiver: Pubkey,
    ) -> Result<()> {
        instructions::admin::handler_set_default_fee_receiver(ctx, new_default_fee_receiver)
    }

    pub fn set_fee_receiver(ctx: Context<SetFeeReceiver>, receiver: Pubkey) -> Result<()> {
        instructions::admin::handler_set_fee_receiver(ctx, receiver)
    }

    pub fn set_allowed_token(ctx: Context<SetAllowedToken>, is_allowed: bool) -> Result<()> {
        instructions::admin::handler_set_allowed_token(ctx, is_allowed)
    }

    pub fn set_order_book(ctx: Context<SetOrderBook>, new_order_book: Pubkey) -> Result<()> {
        instructions::admin::handler_set_order_book(ctx, new_order_book)
    }

    pub fn set_exchange_owner(
        ctx: Context<SetExchangeOwner>,
        new_exchange_owner: Pubkey,
    ) -> Result<()> {
        instructions::admin::handler_set_exchange_owner(ctx, new_exchange_owner)
    }

    pub fn toggle_pause(ctx: Context<TogglePause>) -> Result<()> {
        instructions::admin::handler_toggle_pause(ctx)
    }

    pub fn set_royalties_registry_program(
        ctx: Context<SetRoyaltiesRegistryProgram>,
        new_royalties_registry_program: Pubkey,
    ) -> Result<()> {
        instructions::admin::handler_set_royalties_registry_program(
            ctx,
            new_royalties_registry_program,
        )
    }

    pub fn safe_transfer_spl(ctx: Context<SafeTransferSpl>, amount: u64) -> Result<()> {
        instructions::admin::handler_safe_transfer_spl(ctx, amount)
    }
}
