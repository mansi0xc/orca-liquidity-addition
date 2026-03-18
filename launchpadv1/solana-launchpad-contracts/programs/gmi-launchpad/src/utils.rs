use anchor_lang::prelude::*;
use crate::errors::LaunchpadError;
use crate::state::CollectionType;

/// Calculate the payment required for a mint operation.
/// For Refundable80 with remint available, the price is 80% of the original.
pub fn calculate_payment(
    collection_type: CollectionType,
    price: u64,
    quantity: u64,
    is_remint: bool,
) -> Result<u64> {
    if is_remint && collection_type == CollectionType::Refundable80 {
        // Remint at 80% price
        let discounted_price = price
            .checked_mul(80)
            .ok_or(LaunchpadError::ArithmeticOverflow)?
            .checked_div(100)
            .ok_or(LaunchpadError::ArithmeticOverflow)?;
        discounted_price
            .checked_mul(quantity)
            .ok_or(LaunchpadError::ArithmeticOverflow.into())
    } else {
        price
            .checked_mul(quantity)
            .ok_or(LaunchpadError::ArithmeticOverflow.into())
    }
}

/// Calculate the refund price to store for a given collection type and mint price.
pub fn calculate_refund_price(collection_type: CollectionType, price: u64) -> Result<u64> {
    match collection_type {
        CollectionType::Standard => Ok(0), // No refund for standard
        CollectionType::Refundable100 => Ok(price), // Full refund
        CollectionType::Refundable80 => {
            // 80% of price stored as refund
            price
                .checked_mul(80)
                .ok_or(LaunchpadError::ArithmeticOverflow)?
                .checked_div(100)
                .ok_or(LaunchpadError::ArithmeticOverflow.into())
        }
    }
}

/// Calculate the owner's cut for immediate transfer (only for Refundable80 on fresh mints).
pub fn calculate_owner_cut(collection_type: CollectionType, price: u64) -> Result<u64> {
    match collection_type {
        CollectionType::Standard => Ok(price), // All to owner immediately
        CollectionType::Refundable100 => Ok(0), // Nothing to owner, all in vault
        CollectionType::Refundable80 => {
            // 20% to owner
            price
                .checked_mul(20)
                .ok_or(LaunchpadError::ArithmeticOverflow)?
                .checked_div(100)
                .ok_or(LaunchpadError::ArithmeticOverflow.into())
        }
    }
}

/// Calculate the reserved NFTs (20% of max supply) for Refundable variants.
pub fn calculate_reserved_nfts(max_supply: u64) -> Result<u64> {
    max_supply
        .checked_mul(20)
        .ok_or(LaunchpadError::ArithmeticOverflow)?
        .checked_div(100)
        .ok_or(LaunchpadError::ArithmeticOverflow.into())
}

/// Transfer SOL from one account to another using system program CPI.
pub fn transfer_sol<'info>(
    from: &AccountInfo<'info>,
    to: &AccountInfo<'info>,
    system_program: &AccountInfo<'info>,
    amount: u64,
    signer_seeds: Option<&[&[&[u8]]]>,
) -> Result<()> {
    if amount == 0 {
        return Ok(());
    }

    let ix = anchor_lang::solana_program::system_instruction::transfer(
        from.key,
        to.key,
        amount,
    );

    if let Some(seeds) = signer_seeds {
        anchor_lang::solana_program::program::invoke_signed(
            &ix,
            &[from.clone(), to.clone(), system_program.clone()],
            seeds,
        )?;
    } else {
        anchor_lang::solana_program::program::invoke(
            &ix,
            &[from.clone(), to.clone(), system_program.clone()],
        )?;
    }

    Ok(())
}

/// Transfer SOL from a PDA vault (program-signed) to a recipient.
pub fn transfer_sol_from_vault<'info>(
    vault: &AccountInfo<'info>,
    to: &AccountInfo<'info>,
    amount: u64,
) -> Result<()> {
    if amount == 0 {
        return Ok(());
    }

    // Direct lamport manipulation for PDA-owned accounts
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
