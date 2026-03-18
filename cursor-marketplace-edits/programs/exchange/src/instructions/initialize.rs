use anchor_lang::prelude::*;

use crate::errors::ExchangeError;
use crate::state::ExchangeConfig;

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct InitializeArgs {
    pub order_book: Pubkey,
    pub default_fee_receiver: Pubkey,
    pub royalties_registry_program: Pubkey,
    pub wsol_mint: Pubkey,
    pub exchange_owner: Pubkey,
    pub protocol_fee_bps: u16,
}

#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(
        init,
        payer = authority,
        space = 8 + ExchangeConfig::INIT_SPACE,
        seeds = [b"exchange_config"],
        bump,
    )]
    pub exchange_config: Account<'info, ExchangeConfig>,

    #[account(mut)]
    pub authority: Signer<'info>,

    pub system_program: Program<'info, System>,
}

pub fn handler_initialize(ctx: Context<Initialize>, args: InitializeArgs) -> Result<()> {
    require!(args.protocol_fee_bps <= 10000, ExchangeError::InvalidProtocolFee);

    let config = &mut ctx.accounts.exchange_config;
    config.owner = ctx.accounts.authority.key();
    config.exchange_owner = args.exchange_owner;
    config.order_book = args.order_book;
    config.default_fee_receiver = args.default_fee_receiver;
    config.royalties_registry_program = args.royalties_registry_program;
    config.wsol_mint = args.wsol_mint;
    config.protocol_fee_bps = args.protocol_fee_bps;
    config.is_paused = false;
    config.bump = ctx.bumps.exchange_config;

    Ok(())
}
