use anchor_lang::prelude::*;

use crate::errors::ExchangeError;
use crate::events::CancelOrderEvent;
use crate::state::{ExchangeConfig, OrderFill};
use crate::state::types::Order;
use crate::logic::order::compute_order_key_hash;

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct CancelOrderArgs {
    pub order_key_hash: [u8; 32],
    pub order: Order,
}

#[derive(Accounts)]
#[instruction(args: CancelOrderArgs)]
pub struct CancelOrder<'info> {
    #[account(
        seeds = [b"exchange_config"],
        bump = exchange_config.bump,
    )]
    pub exchange_config: Account<'info, ExchangeConfig>,

    /// The order maker. Must be a signer (RULE SG-1).
    pub maker: Signer<'info>,

    /// The order fill PDA. Initialized if it doesn't exist yet.
    #[account(
        init_if_needed,
        payer = payer,
        space = 8 + OrderFill::INIT_SPACE,
        seeds = [b"order_fill", args.order_key_hash.as_ref()],
        bump,
    )]
    pub order_fill: Account<'info, OrderFill>,

    #[account(mut)]
    pub payer: Signer<'info>,

    pub system_program: Program<'info, System>,
}

pub fn handler_cancel_order(ctx: Context<CancelOrder>, args: CancelOrderArgs) -> Result<()> {
    // RULE PC-1: pause check first
    require!(
        !ctx.accounts.exchange_config.is_paused,
        ExchangeError::Paused
    );

    // RULE SG-1: verify maker is signer and matches order.maker
    require!(
        ctx.accounts.maker.key() == args.order.maker,
        ExchangeError::NotOrderMaker
    );

    // Salt must be non-zero for cancellation
    require!(args.order.salt != 0, ExchangeError::ZeroSaltCannotCancel);

    let order_key_hash = compute_order_key_hash(&args.order);

    // Verify provided hash matches computed hash
    require!(
        order_key_hash == args.order_key_hash,
        ExchangeError::InvalidSignature
    );

    // RULE FM-1: set fill to MAX (permanent cancellation)
    let order_fill = &mut ctx.accounts.order_fill;
    order_fill.fill_amount = u64::MAX;
    order_fill.bump = ctx.bumps.order_fill;

    emit!(CancelOrderEvent {
        order_key_hash,
        maker: args.order.maker,
    });

    Ok(())
}
