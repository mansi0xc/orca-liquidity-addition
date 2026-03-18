use anchor_lang::prelude::*;
use crate::errors::LaunchpadError;
use crate::state::CollectionType;

pub fn calculate_protocol_fee(price: u64, share_bps: u64) -> Result<u64> {
    if share_bps == 0 || price == 0 {
        return Ok(0);
    }
    let fee = (price as u128)
        .checked_mul(share_bps as u128)
        .ok_or(LaunchpadError::ArithmeticOverflow)?
        .checked_div(10000)
        .ok_or(LaunchpadError::PriceTruncationError)? as u64;
    Ok(fee)
}

pub fn calculate_vault_and_owner_cut(
    collection_type: CollectionType,
    net_price: u64,
) -> Result<(u64, u64)> {
    match collection_type {
        CollectionType::Standard => Ok((0, net_price)),
        CollectionType::Refundable100 => Ok((net_price, 0)),
        CollectionType::Refundable80 => {
            let vault_cut = (net_price as u128)
                .checked_mul(80)
                .ok_or(LaunchpadError::ArithmeticOverflow)?
                .checked_div(100)
                .ok_or(LaunchpadError::PriceTruncationError)? as u64;
            
            // Subtractive remainder guarantees exact lamport accounting
            let owner_cut = net_price
                .checked_sub(vault_cut)
                .ok_or(LaunchpadError::ArithmeticUnderflow)?;
                
            Ok((vault_cut, owner_cut))
        }
    }
}

pub fn calculate_reserved_nfts(max_supply: u64) -> Result<u64> {
    (max_supply as u128)
        .checked_mul(20)
        .ok_or(LaunchpadError::ArithmeticOverflow)?
        .checked_div(100)
        .map(|v| v as u64)
        .ok_or(LaunchpadError::ArithmeticOverflow.into())
}

pub fn transfer_sol<'info>(
    from: &AccountInfo<'info>,
    to: &AccountInfo<'info>,
    system_program: &AccountInfo<'info>,
    amount: u64,
) -> Result<()> {
    if amount == 0 {
        return Ok(());
    }

    let ix = anchor_lang::solana_program::system_instruction::transfer(
        from.key,
        to.key,
        amount,
    );

    anchor_lang::solana_program::program::invoke(
        &ix,
        &[from.clone(), to.clone(), system_program.clone()],
    )?;

    Ok(())
}

pub fn transfer_sol_from_vault<'info>(
    vault: &AccountInfo<'info>,
    to: &AccountInfo<'info>,
    amount: u64,
) -> Result<()> {
    if amount == 0 {
        return Ok(());
    }

    **vault.try_borrow_mut_lamports()? = vault
        .lamports()
        .checked_sub(amount)
        .ok_or(LaunchpadError::InsufficientVaultBalance)?;

    **to.try_borrow_mut_lamports()? = to
        .lamports()
        .checked_add(amount)
        .ok_or(LaunchpadError::ArithmeticOverflow)?;

    Ok(())
}

// Bitmap Utilities
pub fn set_refunded_bit(bitmap_account: &mut crate::state::RefundBitmap, token_index: u64) -> Result<()> {
    let byte_idx = (token_index / 8) as usize;
    let bit_idx = (token_index % 8) as u8;
    
    if byte_idx >= 1250 {
        return err!(LaunchpadError::MaxSupply);
    }
    
    bitmap_account.bitmap[byte_idx] |= 1 << bit_idx;
    
    if (byte_idx as u16) < bitmap_account.search_cursor {
        bitmap_account.search_cursor = byte_idx as u16;
    }
    
    Ok(())
}

pub fn claim_next_available_remint(bitmap_account: &mut crate::state::RefundBitmap) -> Result<u64> {
    for i in (bitmap_account.search_cursor as usize)..1250 {
        let byte = bitmap_account.bitmap[i];
        if byte != 0 {
            for bit_idx in 0..8 {
                if (byte & (1 << bit_idx)) != 0 {
                    bitmap_account.bitmap[i] &= !(1 << bit_idx);
                    bitmap_account.search_cursor = i as u16;
                    return Ok((i as u64 * 8) + bit_idx as u64);
                }
            }
        }
    }
    err!(LaunchpadError::ArithmeticUnderflow)
}
