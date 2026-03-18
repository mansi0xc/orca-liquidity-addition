use anchor_lang::prelude::*;
use anchor_lang::solana_program::{self, system_instruction};
use anchor_spl::token::{self, Transfer};

use crate::errors::ExchangeError;
use crate::state::ExchangeConfig;
use crate::state::types::*;
use crate::logic::{bps, exchange::{sub_fee, sub_fee_in_bps, calculate_total_amount}};

struct AccountWalker<'a, 'info> {
    accounts: &'a [AccountInfo<'info>],
    index: usize,
}

impl<'a, 'info> AccountWalker<'a, 'info> {
    fn new(accounts: &'a [AccountInfo<'info>]) -> Self {
        Self { accounts, index: 0 }
    }

    fn next(&mut self) -> Result<&'a AccountInfo<'info>> {
        require!(
            self.index < self.accounts.len(),
            ExchangeError::InsufficientRemainingAccounts
        );
        let acc = &self.accounts[self.index];
        self.index += 1;
        Ok(acc)
    }

    fn next_validated(&mut self, expected_key: &Pubkey) -> Result<&'a AccountInfo<'info>> {
        let acc = self.next()?;
        require!(
            acc.key() == *expected_key,
            ExchangeError::InvalidRemainingAccounts
        );
        Ok(acc)
    }
}

fn spl_transfer<'info>(
    source: &AccountInfo<'info>,
    destination: &AccountInfo<'info>,
    authority: &AccountInfo<'info>,
    token_program: &AccountInfo<'info>,
    authority_bump: u8,
    amount: u64,
) -> Result<()> {
    if amount == 0 {
        return Ok(());
    }
    let seeds = &[b"exchange_authority".as_ref(), &[authority_bump]];
    let signer_seeds = &[&seeds[..]];
    let cpi_accounts = Transfer {
        from: source.clone(),
        to: destination.clone(),
        authority: authority.clone(),
    };
    let cpi_ctx = CpiContext::new_with_signer(token_program.clone(), cpi_accounts, signer_seeds);
    token::transfer(cpi_ctx, amount)
}

fn sol_transfer<'info>(
    from: &AccountInfo<'info>,
    to: &AccountInfo<'info>,
    amount: u64,
) -> Result<()> {
    if amount == 0 {
        return Ok(());
    }
    let ix = system_instruction::transfer(from.key, to.key, amount);
    solana_program::program::invoke(&ix, &[from.clone(), to.clone()])
        .map_err(|_| error!(ExchangeError::TransferFailed))
}

fn do_transfer<'info>(
    source: &AccountInfo<'info>,
    destination: &AccountInfo<'info>,
    exchange_authority: &AccountInfo<'info>,
    token_program: &AccountInfo<'info>,
    authority_bump: u8,
    amount: u64,
    is_sol: bool,
) -> Result<()> {
    if amount == 0 {
        return Ok(());
    }
    require!(
        destination.key() != Pubkey::default(),
        ExchangeError::ZeroAddressTransfer
    );
    if is_sol {
        sol_transfer(source, destination, amount)
    } else {
        spl_transfer(source, destination, exchange_authority, token_program, authority_bump, amount)
    }
}

/// Resolves the expected fee receiver pubkey from config and optional FeeReceiver PDA.
/// If `fee_receiver_pda` is Some and initialized, use its `receiver`; otherwise use
/// `config.default_fee_receiver`.
pub fn resolve_fee_receiver(
    config: &ExchangeConfig,
    fee_receiver_override: Option<&Pubkey>,
) -> Pubkey {
    fee_receiver_override.copied().unwrap_or(config.default_fee_receiver)
}

/// Full transfer pipeline for a matched order pair.
///
/// # remaining_accounts layout
///
/// ## When fee_side is Make or Take:
///
/// | Index | Account |
/// |-------|---------|
/// | 0 | fee_payer_source (ATA for SPL / wallet for SOL) |
/// | 1 | protocol_fee_receiver_dest (VALIDATED against config) |
/// | 2..2+R | royalty_recipient_dests (VALIDATED against royalty_parts) |
/// | 2+R..2+R+O1 | fee_payer origin_fee dests (VALIDATED against order data) |
/// | 2+R+O1..+O2 | other origin_fee dests (VALIDATED against order data) |
/// | ...+P1 | other_order payout dests (VALIDATED against order data) |
/// | next | non_fee_source (ATA for non-fee asset, e.g. NFT) |
/// | next+1..+P2 | fee_payer_order payout dests (VALIDATED against order data) |
///
/// ## When fee_side is None:
///
/// | Index | Account |
/// |-------|---------|
/// | 0 | source_make (ATA for make asset) |
/// | 1..1+P_L | left_order payout dests (VALIDATED) |
/// | 1+P_L | source_take (ATA for take asset) |
/// | 2+P_L..+P_R | right_order payout dests (VALIDATED) |
#[allow(clippy::too_many_arguments)]
pub fn execute_transfers<'info>(
    remaining_accounts: &[AccountInfo<'info>],
    _order_left: &Order,
    _order_right: &Order,
    left_order_data: &DataV1,
    right_order_data: &DataV1,
    maker_asset_type: &AssetType,
    taker_asset_type: &AssetType,
    fee_side: &FeeSide,
    new_fill: &FillResult,
    config: &ExchangeConfig,
    exchange_authority: &AccountInfo<'info>,
    token_program: &AccountInfo<'info>,
    payer: &AccountInfo<'info>,
    exchange_authority_bump: u8,
    royalty_parts: &[Part],
    expected_fee_receiver: &Pubkey,
) -> Result<()> {
    validate_payout_sum(&left_order_data.payouts)?;
    validate_payout_sum(&right_order_data.payouts)?;

    let mut walker = AccountWalker::new(remaining_accounts);

    match fee_side {
        FeeSide::Make => {
            let amount = new_fill.left_order_take_value;
            let is_sol = matches!(maker_asset_type.asset_class, AssetClass::Sol);

            do_transfers_with_fees(
                &mut walker,
                amount,
                right_order_data,
                left_order_data,
                config,
                exchange_authority,
                token_program,
                exchange_authority_bump,
                royalty_parts,
                is_sol,
                expected_fee_receiver,
            )?;

            let nft_amount = new_fill.right_order_take_value;
            let is_nft_sol = matches!(taker_asset_type.asset_class, AssetClass::Sol);
            do_transfer_payouts(
                &mut walker,
                nft_amount,
                &right_order_data.payouts,
                exchange_authority,
                token_program,
                payer,
                exchange_authority_bump,
                is_nft_sol,
            )?;
        }
        FeeSide::Take => {
            let amount = new_fill.right_order_take_value;
            let is_sol = matches!(taker_asset_type.asset_class, AssetClass::Sol);

            do_transfers_with_fees(
                &mut walker,
                amount,
                left_order_data,
                right_order_data,
                config,
                exchange_authority,
                token_program,
                exchange_authority_bump,
                royalty_parts,
                is_sol,
                expected_fee_receiver,
            )?;

            let nft_amount = new_fill.left_order_take_value;
            let is_nft_sol = matches!(maker_asset_type.asset_class, AssetClass::Sol);
            do_transfer_payouts(
                &mut walker,
                nft_amount,
                &left_order_data.payouts,
                exchange_authority,
                token_program,
                payer,
                exchange_authority_bump,
                is_nft_sol,
            )?;
        }
        FeeSide::None => {
            do_transfer_payouts(
                &mut walker,
                new_fill.left_order_take_value,
                &left_order_data.payouts,
                exchange_authority,
                token_program,
                payer,
                exchange_authority_bump,
                matches!(maker_asset_type.asset_class, AssetClass::Sol),
            )?;

            do_transfer_payouts(
                &mut walker,
                new_fill.right_order_take_value,
                &right_order_data.payouts,
                exchange_authority,
                token_program,
                payer,
                exchange_authority_bump,
                matches!(taker_asset_type.asset_class, AssetClass::Sol),
            )?;
        }
    }

    Ok(())
}

fn validate_payout_sum(payouts: &[Part]) -> Result<()> {
    let sum: u64 = payouts.iter().map(|p| p.value as u64).sum();
    require!(sum == 10000, ExchangeError::InvalidPayoutSum);
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn do_transfers_with_fees<'a, 'info>(
    walker: &mut AccountWalker<'a, 'info>,
    amount: u64,
    fee_payer_data: &DataV1,
    other_data: &DataV1,
    config: &ExchangeConfig,
    exchange_authority: &AccountInfo<'info>,
    token_program: &AccountInfo<'info>,
    authority_bump: u8,
    royalty_parts: &[Part],
    is_sol: bool,
    expected_fee_receiver: &Pubkey,
) -> Result<()> {
    let source = walker.next()?;

    let total_amount = calculate_total_amount(amount, &fee_payer_data.origin_fees)?;

    // FIX C2: Validate fee receiver matches on-chain config
    let fee_dest = walker.next_validated(expected_fee_receiver)?;
    let (mut rest, protocol_fee) = sub_fee_in_bps(total_amount, amount, config.protocol_fee_bps)?;
    if protocol_fee > 0 {
        do_transfer(
            source, fee_dest, exchange_authority, token_program,
            authority_bump, protocol_fee, is_sol,
        )?;
    }

    // FIX C1/C5: Validate royalty destinations match royalty_parts
    let mut total_royalties_bps: u64 = 0;
    for royalty in royalty_parts {
        let royalty_dest = walker.next_validated(&royalty.account)?;
        require!(
            total_royalties_bps + royalty.value as u64 <= 5000,
            ExchangeError::RoyaltiesTooHigh
        );
        total_royalties_bps += royalty.value as u64;

        let royalty_amount = bps::bps(amount, royalty.value)?;
        let (new_rest, actual) = sub_fee(rest, royalty_amount);
        rest = new_rest;
        if actual > 0 {
            do_transfer(
                source, royalty_dest, exchange_authority, token_program,
                authority_bump, actual, is_sol,
            )?;
        }
    }

    // FIX C5: Validate origin fee destinations match order data
    for fee in &fee_payer_data.origin_fees {
        let origin_dest = walker.next_validated(&fee.account)?;
        let fee_amount = bps::bps(amount, fee.value)?;
        let (new_rest, actual) = sub_fee(rest, fee_amount);
        rest = new_rest;
        if actual > 0 {
            do_transfer(
                source, origin_dest, exchange_authority, token_program,
                authority_bump, actual, is_sol,
            )?;
        }
    }

    for fee in &other_data.origin_fees {
        let origin_dest = walker.next_validated(&fee.account)?;
        let fee_amount = bps::bps(amount, fee.value)?;
        let (new_rest, actual) = sub_fee(rest, fee_amount);
        rest = new_rest;
        if actual > 0 {
            do_transfer(
                source, origin_dest, exchange_authority, token_program,
                authority_bump, actual, is_sol,
            )?;
        }
    }

    // FIX C3: Validate payout destinations match order data
    do_payouts(
        walker, source, rest, &other_data.payouts,
        exchange_authority, token_program, authority_bump, is_sol,
    )
}

#[allow(clippy::too_many_arguments)]
fn do_transfer_payouts<'a, 'info>(
    walker: &mut AccountWalker<'a, 'info>,
    amount: u64,
    payouts: &[Part],
    exchange_authority: &AccountInfo<'info>,
    token_program: &AccountInfo<'info>,
    _payer: &AccountInfo<'info>,
    authority_bump: u8,
    is_sol: bool,
) -> Result<()> {
    let source = walker.next()?;
    do_payouts(
        walker, source, amount, payouts,
        exchange_authority, token_program, authority_bump, is_sol,
    )
}

#[allow(clippy::too_many_arguments)]
fn do_payouts<'a, 'info>(
    walker: &mut AccountWalker<'a, 'info>,
    source: &AccountInfo<'info>,
    amount: u64,
    payouts: &[Part],
    exchange_authority: &AccountInfo<'info>,
    token_program: &AccountInfo<'info>,
    authority_bump: u8,
    is_sol: bool,
) -> Result<()> {
    if payouts.is_empty() || amount == 0 {
        return Ok(());
    }

    let mut rest = amount;
    let last_idx = payouts.len() - 1;

    for (i, payout) in payouts.iter().enumerate() {
        // FIX C3: Validate payout destination matches signed order data
        let dest = walker.next_validated(&payout.account)?;

        let transfer_amount = if i < last_idx {
            let calculated = bps::bps(amount, payout.value)?;
            let actual = std::cmp::min(calculated, rest);
            rest = rest.saturating_sub(actual);
            actual
        } else {
            rest
        };

        do_transfer(
            source, dest, exchange_authority, token_program,
            authority_bump, transfer_amount, is_sol,
        )?;
    }

    Ok(())
}
