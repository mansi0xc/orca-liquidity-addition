use anchor_lang::prelude::*;
use anchor_lang::solana_program;
use anchor_spl::token::Token;

use crate::errors::ExchangeError;
use crate::events::MatchEvent;
use crate::state::{ExchangeConfig, OrderFill};
use crate::state::types::*;
use crate::logic::{
    order::{compute_order_key_hash, compute_order_hash, compute_match_allowance_hash, validate_order},
    fill::fill_order,
    fee_side::get_fee_side,
    exchange::{match_order_assets, check_counterparties},
    order_data::parse_order_data,
    signature::verify_ed25519_signature,
    transfers::execute_transfers,
};

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct MatchOrdersArgs {
    pub left_order_key_hash: [u8; 32],
    pub right_order_key_hash: [u8; 32],
    pub order_left: Order,
    pub signature_left: Vec<u8>,
    pub match_left_before_timestamp: i64,
    pub order_book_signature_left: Vec<u8>,
    pub order_right: Order,
    pub signature_right: Vec<u8>,
    pub match_right_before_timestamp: i64,
    pub order_book_signature_right: Vec<u8>,
    pub royalty_parts: Vec<Part>,
}

#[derive(Accounts)]
#[instruction(args: MatchOrdersArgs)]
pub struct MatchOrders<'info> {
    #[account(
        seeds = [b"exchange_config"],
        bump = exchange_config.bump,
    )]
    pub exchange_config: Account<'info, ExchangeConfig>,

    #[account(mut)]
    pub payer: Signer<'info>,

    #[account(
        init_if_needed,
        payer = payer,
        space = 8 + OrderFill::INIT_SPACE,
        seeds = [b"order_fill", args.left_order_key_hash.as_ref()],
        bump,
    )]
    pub left_order_fill: Account<'info, OrderFill>,

    #[account(
        init_if_needed,
        payer = payer,
        space = 8 + OrderFill::INIT_SPACE,
        seeds = [b"order_fill", args.right_order_key_hash.as_ref()],
        bump,
    )]
    pub right_order_fill: Account<'info, OrderFill>,

    /// CHECK: Instructions sysvar for Ed25519 signature verification introspection.
    #[account(address = solana_program::sysvar::instructions::ID)]
    pub instructions_sysvar: AccountInfo<'info>,

    /// CHECK: Exchange authority PDA used as delegate for token transfers.
    #[account(
        seeds = [b"exchange_authority"],
        bump,
    )]
    pub exchange_authority: AccountInfo<'info>,

    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
    // Dynamic accounts are passed via remaining_accounts.
    // See execute_transfers for the expected layout.
}

pub fn handler_match_orders<'info>(
    ctx: Context<'_, '_, 'info, 'info, MatchOrders<'info>>,
    mut args: MatchOrdersArgs,
) -> Result<()> {
    let config = &ctx.accounts.exchange_config;
    let program_id = ctx.program_id;
    let clock = Clock::get()?;

    require!(!config.is_paused, ExchangeError::Paused);

    require!(
        args.order_right.make_asset.asset_type.asset_class != AssetClass::Sol,
        ExchangeError::MakerCannotPayWithSol
    );

    require!(
        !args.order_left.collection_bid && !args.order_right.collection_bid,
        ExchangeError::CollectionBidMustUseCollectionBidInstruction
    );

    validate_order(&args.order_left, clock.unix_timestamp)?;
    validate_order(&args.order_right, clock.unix_timestamp)?;

    // FIX H4: Validate NFT value == 1
    validate_nft_values(&args.order_left)?;
    validate_nft_values(&args.order_right)?;

    check_counterparties(&args.order_left, &args.order_right)?;

    // FIX C4: Validate SPL token whitelist
    validate_spl_token_whitelist(&args.order_left, ctx.remaining_accounts, config)?;
    validate_spl_token_whitelist(&args.order_right, ctx.remaining_accounts, config)?;

    let left_key_hash = compute_order_key_hash(&args.order_left);
    let right_key_hash = compute_order_key_hash(&args.order_right);
    require!(left_key_hash == args.left_order_key_hash, ExchangeError::InvalidSignature);
    require!(right_key_hash == args.right_order_key_hash, ExchangeError::InvalidSignature);

    // FIX M4: Assign maker to payer for zero-salt orders with default maker
    if args.order_left.salt == 0 && args.order_left.maker == Pubkey::default() {
        args.order_left.maker = ctx.accounts.payer.key();
    }
    if args.order_right.salt == 0 && args.order_right.maker == Pubkey::default() {
        args.order_right.maker = ctx.accounts.payer.key();
    }

    verify_order_signatures(
        &ctx.accounts.instructions_sysvar,
        &args,
        &left_key_hash,
        &right_key_hash,
        config,
        program_id,
        &clock,
        &ctx.accounts.payer,
    )?;

    let (maker_asset_type, taker_asset_type) =
        match_order_assets(&args.order_left, &args.order_right)?;

    let left_fill_amount = if args.order_left.salt == 0 {
        0u64
    } else {
        ctx.accounts.left_order_fill.fill_amount
    };
    let right_fill_amount = if args.order_right.salt == 0 {
        0u64
    } else {
        ctx.accounts.right_order_fill.fill_amount
    };

    let new_fill = fill_order(
        &args.order_left,
        &args.order_right,
        left_fill_amount,
        right_fill_amount,
    )?;

    require!(new_fill.left_order_take_value > 0, ExchangeError::NothingToFill);

    if args.order_left.salt != 0 {
        let left_fill = &mut ctx.accounts.left_order_fill;
        left_fill.fill_amount = left_fill_amount
            .checked_add(new_fill.left_order_take_value)
            .ok_or(ExchangeError::FillOverflow)?;
        left_fill.bump = ctx.bumps.left_order_fill;
    }
    if args.order_right.salt != 0 {
        let right_fill = &mut ctx.accounts.right_order_fill;
        right_fill.fill_amount = right_fill_amount
            .checked_add(new_fill.right_order_take_value)
            .ok_or(ExchangeError::FillOverflow)?;
        right_fill.bump = ctx.bumps.right_order_fill;
    }

    let fee_side = get_fee_side(
        &maker_asset_type.asset_class,
        &taker_asset_type.asset_class,
    );

    let left_order_data = parse_order_data(&args.order_left)?;
    let right_order_data = parse_order_data(&args.order_right)?;

    // FIX C2: Resolve and validate fee receiver from on-chain config
    let expected_fee_receiver = config.default_fee_receiver;

    execute_transfers(
        ctx.remaining_accounts,
        &args.order_left,
        &args.order_right,
        &left_order_data,
        &right_order_data,
        &maker_asset_type,
        &taker_asset_type,
        &fee_side,
        &new_fill,
        config,
        &ctx.accounts.exchange_authority,
        &ctx.accounts.token_program.to_account_info(),
        &ctx.accounts.payer.to_account_info(),
        ctx.bumps.exchange_authority,
        &args.royalty_parts,
        &expected_fee_receiver,
    )?;

    emit!(MatchEvent {
        left_order_key_hash: left_key_hash,
        right_order_key_hash: right_key_hash,
        left_maker: args.order_left.maker,
        right_maker: args.order_right.maker,
        new_left_fill: new_fill.left_order_take_value,
        new_right_fill: new_fill.right_order_take_value,
    });

    Ok(())
}

/// FIX H4: Validate that NFT assets have value == 1.
fn validate_nft_values(order: &Order) -> Result<()> {
    if order.make_asset.asset_type.asset_class == AssetClass::Nft {
        require!(
            order.make_asset.value == 1,
            ExchangeError::AssetClassMismatch
        );
    }
    if order.take_asset.asset_type.asset_class == AssetClass::Nft {
        require!(
            order.take_asset.value == 1,
            ExchangeError::AssetClassMismatch
        );
    }
    Ok(())
}

/// FIX C4: Validate SPL tokens are whitelisted.
/// AllowedToken PDAs must be passed in remaining_accounts. We search for them
/// by deriving the expected PDA address and matching against remaining_accounts.
fn validate_spl_token_whitelist(
    order: &Order,
    remaining_accounts: &[AccountInfo],
    _config: &ExchangeConfig,
) -> Result<()> {
    if order.make_asset.asset_type.asset_class == AssetClass::SplToken {
        check_token_allowed(&order.make_asset.asset_type.mint, remaining_accounts)?;
    }
    if order.take_asset.asset_type.asset_class == AssetClass::SplToken {
        check_token_allowed(&order.take_asset.asset_type.mint, remaining_accounts)?;
    }
    Ok(())
}

/// Verify that an AllowedToken PDA exists in remaining_accounts and is_allowed.
fn check_token_allowed(mint: &Pubkey, remaining_accounts: &[AccountInfo]) -> Result<()> {
    let (expected_pda, _bump) = Pubkey::find_program_address(
        &[b"allowed_token", mint.as_ref()],
        &crate::ID,
    );

    for account in remaining_accounts {
        if account.key() == expected_pda {
            let data = account.try_borrow_data()?;
            if data.len() >= 8 + 1 {
                // Skip 8-byte discriminator, read is_allowed (1 byte)
                let is_allowed = data[8] != 0;
                require!(is_allowed, ExchangeError::TokenNotAllowed);
                return Ok(());
            }
        }
    }

    Err(ExchangeError::TokenNotAllowed.into())
}

fn verify_order_signatures(
    instructions_sysvar: &AccountInfo,
    args: &MatchOrdersArgs,
    left_key_hash: &[u8; 32],
    right_key_hash: &[u8; 32],
    config: &ExchangeConfig,
    program_id: &Pubkey,
    clock: &Clock,
    payer: &Signer,
) -> Result<()> {
    let mut sig_ix_index: usize = 0;

    if args.order_left.salt > 0 {
        require!(
            args.match_left_before_timestamp > clock.unix_timestamp,
            ExchangeError::MatchAllowanceExpired
        );
        let match_allowance_hash = compute_match_allowance_hash(
            left_key_hash,
            args.match_left_before_timestamp,
            program_id,
        );
        verify_ed25519_signature(
            instructions_sysvar,
            &config.order_book,
            &match_allowance_hash,
            sig_ix_index,
        )?;
        sig_ix_index += 1;

        if payer.key() != args.order_left.maker {
            let order_hash = compute_order_hash(&args.order_left, program_id);
            verify_ed25519_signature(
                instructions_sysvar,
                &args.order_left.maker,
                &order_hash,
                sig_ix_index,
            )?;
            sig_ix_index += 1;
        }
    } else {
        if args.order_left.maker != Pubkey::default() {
            require!(
                payer.key() == args.order_left.maker,
                ExchangeError::MakerMustBeSignerForZeroSalt
            );
        }
    }

    if args.order_right.salt > 0 {
        require!(
            args.match_right_before_timestamp > clock.unix_timestamp,
            ExchangeError::MatchAllowanceExpired
        );
        let match_allowance_hash = compute_match_allowance_hash(
            right_key_hash,
            args.match_right_before_timestamp,
            program_id,
        );
        verify_ed25519_signature(
            instructions_sysvar,
            &config.order_book,
            &match_allowance_hash,
            sig_ix_index,
        )?;
        sig_ix_index += 1;

        if payer.key() != args.order_right.maker {
            let order_hash = compute_order_hash(&args.order_right, program_id);
            verify_ed25519_signature(
                instructions_sysvar,
                &args.order_right.maker,
                &order_hash,
                sig_ix_index,
            )?;
            let _ = sig_ix_index;
        }
    } else {
        if args.order_right.maker != Pubkey::default() {
            require!(
                payer.key() == args.order_right.maker,
                ExchangeError::MakerMustBeSignerForZeroSalt
            );
        }
    }

    Ok(())
}
