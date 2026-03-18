use anchor_lang::prelude::*;
use anchor_spl::token::{self, Mint, Token, TokenAccount, Transfer};

use crate::errors::ExchangeError;
use crate::state::{ExchangeConfig, AllowedToken, FeeReceiver};

// ─── set_protocol_fee_bps ────────────────────────────────────────────────────

#[derive(Accounts)]
pub struct SetProtocolFeeBps<'info> {
    #[account(
        mut,
        seeds = [b"exchange_config"],
        bump = exchange_config.bump,
    )]
    pub exchange_config: Account<'info, ExchangeConfig>,

    #[account(
        constraint = exchange_owner.key() == exchange_config.exchange_owner @ ExchangeError::Unauthorized
    )]
    pub exchange_owner: Signer<'info>,
}

pub fn handler_set_protocol_fee_bps(
    ctx: Context<SetProtocolFeeBps>,
    new_protocol_fee_bps: u16,
) -> Result<()> {
    require!(new_protocol_fee_bps <= 10000, ExchangeError::InvalidProtocolFee);
    ctx.accounts.exchange_config.protocol_fee_bps = new_protocol_fee_bps;
    Ok(())
}

// ─── set_default_fee_receiver ────────────────────────────────────────────────

#[derive(Accounts)]
pub struct SetDefaultFeeReceiver<'info> {
    #[account(
        mut,
        seeds = [b"exchange_config"],
        bump = exchange_config.bump,
    )]
    pub exchange_config: Account<'info, ExchangeConfig>,

    #[account(
        constraint = exchange_owner.key() == exchange_config.exchange_owner @ ExchangeError::Unauthorized
    )]
    pub exchange_owner: Signer<'info>,
}

pub fn handler_set_default_fee_receiver(
    ctx: Context<SetDefaultFeeReceiver>,
    new_default_fee_receiver: Pubkey,
) -> Result<()> {
    ctx.accounts.exchange_config.default_fee_receiver = new_default_fee_receiver;
    Ok(())
}

// ─── set_fee_receiver ────────────────────────────────────────────────────────

#[derive(Accounts)]
#[instruction(receiver: Pubkey)]
pub struct SetFeeReceiver<'info> {
    #[account(
        seeds = [b"exchange_config"],
        bump = exchange_config.bump,
    )]
    pub exchange_config: Account<'info, ExchangeConfig>,

    #[account(
        mut,
        constraint = exchange_owner.key() == exchange_config.exchange_owner @ ExchangeError::Unauthorized
    )]
    pub exchange_owner: Signer<'info>,

    #[account(
        init_if_needed,
        payer = exchange_owner,
        space = 8 + FeeReceiver::INIT_SPACE,
        seeds = [b"fee_receiver", mint.key().as_ref()],
        bump,
    )]
    pub fee_receiver: Account<'info, FeeReceiver>,

    pub mint: Account<'info, Mint>,

    pub system_program: Program<'info, System>,
}

pub fn handler_set_fee_receiver(
    ctx: Context<SetFeeReceiver>,
    receiver: Pubkey,
) -> Result<()> {
    let fee_receiver = &mut ctx.accounts.fee_receiver;
    fee_receiver.receiver = receiver;
    fee_receiver.bump = ctx.bumps.fee_receiver;
    Ok(())
}

// ─── set_allowed_token ───────────────────────────────────────────────────────

#[derive(Accounts)]
#[instruction(is_allowed: bool)]
pub struct SetAllowedToken<'info> {
    #[account(
        seeds = [b"exchange_config"],
        bump = exchange_config.bump,
    )]
    pub exchange_config: Account<'info, ExchangeConfig>,

    #[account(
        mut,
        constraint = exchange_owner.key() == exchange_config.exchange_owner @ ExchangeError::Unauthorized
    )]
    pub exchange_owner: Signer<'info>,

    #[account(
        init_if_needed,
        payer = exchange_owner,
        space = 8 + AllowedToken::INIT_SPACE,
        seeds = [b"allowed_token", mint.key().as_ref()],
        bump,
    )]
    pub allowed_token: Account<'info, AllowedToken>,

    pub mint: Account<'info, Mint>,

    pub system_program: Program<'info, System>,
}

pub fn handler_set_allowed_token(
    ctx: Context<SetAllowedToken>,
    is_allowed: bool,
) -> Result<()> {
    let allowed = &mut ctx.accounts.allowed_token;
    allowed.is_allowed = is_allowed;
    allowed.bump = ctx.bumps.allowed_token;
    Ok(())
}

// ─── set_order_book ──────────────────────────────────────────────────────────

#[derive(Accounts)]
pub struct SetOrderBook<'info> {
    #[account(
        mut,
        seeds = [b"exchange_config"],
        bump = exchange_config.bump,
    )]
    pub exchange_config: Account<'info, ExchangeConfig>,

    #[account(
        constraint = owner.key() == exchange_config.owner @ ExchangeError::Unauthorized
    )]
    pub owner: Signer<'info>,
}

pub fn handler_set_order_book(
    ctx: Context<SetOrderBook>,
    new_order_book: Pubkey,
) -> Result<()> {
    ctx.accounts.exchange_config.order_book = new_order_book;
    Ok(())
}

// ─── set_exchange_owner ──────────────────────────────────────────────────────

#[derive(Accounts)]
pub struct SetExchangeOwner<'info> {
    #[account(
        mut,
        seeds = [b"exchange_config"],
        bump = exchange_config.bump,
    )]
    pub exchange_config: Account<'info, ExchangeConfig>,

    #[account(
        constraint = exchange_owner.key() == exchange_config.exchange_owner @ ExchangeError::Unauthorized
    )]
    pub exchange_owner: Signer<'info>,
}

pub fn handler_set_exchange_owner(
    ctx: Context<SetExchangeOwner>,
    new_exchange_owner: Pubkey,
) -> Result<()> {
    ctx.accounts.exchange_config.exchange_owner = new_exchange_owner;
    Ok(())
}

// ─── toggle_pause ────────────────────────────────────────────────────────────

#[derive(Accounts)]
pub struct TogglePause<'info> {
    #[account(
        mut,
        seeds = [b"exchange_config"],
        bump = exchange_config.bump,
    )]
    pub exchange_config: Account<'info, ExchangeConfig>,

    #[account(
        constraint = owner.key() == exchange_config.owner @ ExchangeError::Unauthorized
    )]
    pub owner: Signer<'info>,
}

pub fn handler_toggle_pause(ctx: Context<TogglePause>) -> Result<()> {
    ctx.accounts.exchange_config.is_paused = !ctx.accounts.exchange_config.is_paused;
    Ok(())
}

// ─── set_royalties_registry_program ──────────────────────────────────────────

#[derive(Accounts)]
pub struct SetRoyaltiesRegistryProgram<'info> {
    #[account(
        mut,
        seeds = [b"exchange_config"],
        bump = exchange_config.bump,
    )]
    pub exchange_config: Account<'info, ExchangeConfig>,

    #[account(
        constraint = owner.key() == exchange_config.owner @ ExchangeError::Unauthorized
    )]
    pub owner: Signer<'info>,
}

pub fn handler_set_royalties_registry_program(
    ctx: Context<SetRoyaltiesRegistryProgram>,
    new_royalties_registry_program: Pubkey,
) -> Result<()> {
    ctx.accounts.exchange_config.royalties_registry_program = new_royalties_registry_program;
    Ok(())
}

// ─── safe_transfer_spl ───────────────────────────────────────────────────────

#[derive(Accounts)]
pub struct SafeTransferSpl<'info> {
    #[account(
        seeds = [b"exchange_config"],
        bump = exchange_config.bump,
    )]
    pub exchange_config: Account<'info, ExchangeConfig>,

    #[account(
        constraint = owner.key() == exchange_config.owner @ ExchangeError::Unauthorized
    )]
    pub owner: Signer<'info>,

    #[account(mut)]
    pub source_token_account: Account<'info, TokenAccount>,

    #[account(mut)]
    pub destination_token_account: Account<'info, TokenAccount>,

    /// The exchange PDA authority that owns the source token account.
    /// CHECK: Verified by seeds below.
    #[account(
        seeds = [b"exchange_authority"],
        bump,
    )]
    pub exchange_authority: AccountInfo<'info>,

    pub token_program: Program<'info, Token>,
}

pub fn handler_safe_transfer_spl(
    ctx: Context<SafeTransferSpl>,
    amount: u64,
) -> Result<()> {
    let seeds = &[
        b"exchange_authority".as_ref(),
        &[ctx.bumps.exchange_authority],
    ];
    let signer_seeds = &[&seeds[..]];

    let cpi_accounts = Transfer {
        from: ctx.accounts.source_token_account.to_account_info(),
        to: ctx.accounts.destination_token_account.to_account_info(),
        authority: ctx.accounts.exchange_authority.to_account_info(),
    };
    let cpi_program = ctx.accounts.token_program.to_account_info();
    let cpi_ctx = CpiContext::new_with_signer(cpi_program, cpi_accounts, signer_seeds);

    token::transfer(cpi_ctx, amount)?;
    Ok(())
}
