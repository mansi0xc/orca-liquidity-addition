//! Whirlpool CPI interface definitions.
//! 
//! This module provides the necessary account structures and CPI helpers
//! for interacting with the Orca Whirlpool program.

use anchor_lang::prelude::*;
use solana_pubkey::pubkey;

/// Orca Whirlpool program ID (mainnet/devnet)
pub const WHIRLPOOL_PROGRAM_ID: Pubkey = 
    pubkey!("whirLbMiicVdio4qvUfM5KAg6Ct8VwpYzGff3uctyCc");

// =============================================================================
// WHIRLPOOL STATE ACCOUNTS
// =============================================================================

/// Whirlpool account structure (subset of fields needed for validation)
/// Note: This is for reading data from Orca Whirlpool accounts, not for our program's accounts.
/// Orca uses Anchor discriminator (8 bytes) at the start.
#[derive(AnchorDeserialize, AnchorSerialize, Clone, Default)]
pub struct Whirlpool {
    pub whirlpools_config: Pubkey,
    pub whirlpool_bump: [u8; 1],
    pub tick_spacing: u16,
    pub tick_spacing_seed: [u8; 2],
    pub fee_rate: u16,
    pub protocol_fee_rate: u16,
    pub liquidity: u128,
    pub sqrt_price: u128,
    pub tick_current_index: i32,
    pub protocol_fee_owed_a: u64,
    pub protocol_fee_owed_b: u64,
    pub token_mint_a: Pubkey,
    pub token_vault_a: Pubkey,
    pub fee_growth_global_a: u128,
    pub token_mint_b: Pubkey,
    pub token_vault_b: Pubkey,
    pub fee_growth_global_b: u128,
    pub reward_last_updated_timestamp: u64,
    pub reward_infos: [WhirlpoolRewardInfo; 3],
}

/// Anchor discriminator for Whirlpool account: sha256("account:Whirlpool")[..8]
pub const WHIRLPOOL_DISCRIMINATOR: [u8; 8] = [63, 149, 209, 12, 225, 128, 99, 9];

impl Whirlpool {
    /// Deserialize a Whirlpool from AccountInfo, skipping the 8-byte discriminator
    pub fn from_account_info(account: &AccountInfo) -> Result<Self> {
        // Verify owner
        require_keys_eq!(
            *account.owner,
            WHIRLPOOL_PROGRAM_ID,
            ErrorCode::AccountOwnedByWrongProgram
        );

        let data = account.try_borrow_data()?;
        // Validate discriminator before deserialization
        require!(data.len() >= 8, ErrorCode::AccountDidNotDeserialize);
        require!(
            data[..8] == WHIRLPOOL_DISCRIMINATOR,
            ErrorCode::AccountDiscriminatorMismatch
        );
        // Skip 8-byte Anchor discriminator
        let whirlpool_data = &data[8..];
        Self::deserialize(&mut &whirlpool_data[..])
            .map_err(|_| error!(ErrorCode::AccountDidNotDeserialize))
    }
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Default, Copy)]
pub struct WhirlpoolRewardInfo {
    pub mint: Pubkey,
    pub vault: Pubkey,
    pub authority: Pubkey,
    pub emissions_per_second_x64: u128,
    pub growth_global_x64: u128,
}

/// Position account structure
#[account]
#[derive(Default)]
pub struct Position {
    pub whirlpool: Pubkey,
    pub position_mint: Pubkey,
    pub liquidity: u128,
    pub tick_lower_index: i32,
    pub tick_upper_index: i32,
    pub fee_growth_checkpoint_a: u128,
    pub fee_owed_a: u64,
    pub fee_growth_checkpoint_b: u128,
    pub fee_owed_b: u64,
    pub reward_infos: [PositionRewardInfo; 3],
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Default, Copy)]
pub struct PositionRewardInfo {
    pub growth_inside_checkpoint: u128,
    pub amount_owed: u64,
}

// =============================================================================
// CPI INSTRUCTION DISCRIMINATORS
// =============================================================================

/// Instruction discriminators for Whirlpool program
pub mod instruction {
    /// open_position instruction discriminator
    pub const OPEN_POSITION: [u8; 8] = [135, 128, 47, 77, 15, 152, 240, 49];
    
    /// open_position_with_metadata instruction discriminator  
    pub const OPEN_POSITION_WITH_METADATA: [u8; 8] = [242, 16, 67, 117, 169, 37, 178, 180];
    
    /// increase_liquidity instruction discriminator
    pub const INCREASE_LIQUIDITY: [u8; 8] = [46, 156, 243, 118, 13, 205, 251, 178];
    
    /// decrease_liquidity instruction discriminator
    pub const DECREASE_LIQUIDITY: [u8; 8] = [160, 38, 208, 111, 172, 137, 242, 17];
    
    /// update_fees_and_rewards instruction discriminator
    pub const UPDATE_FEES_AND_REWARDS: [u8; 8] = [154, 230, 250, 13, 236, 209, 75, 223];

    /// collect_fees instruction discriminator
    pub const COLLECT_FEES: [u8; 8] = [164, 152, 207, 99, 30, 186, 19, 182];
    
    /// close_position instruction discriminator
    pub const CLOSE_POSITION: [u8; 8] = [123, 134, 81, 0, 49, 68, 98, 98];
}

// =============================================================================
// OPEN POSITION CPI
// =============================================================================

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct OpenPositionBumps {
    pub position_bump: u8,
}

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct OpenPositionParams {
    pub bumps: OpenPositionBumps,
    pub tick_lower_index: i32,
    pub tick_upper_index: i32,
}

/// Open a new position in the whirlpool.
/// 
/// # Arguments
/// * `whirlpool_program` - Whirlpool program account
/// * `funder` - Pays for account creation
/// * `owner` - Owner of the position (will own position NFT)
/// * `position` - Position PDA to be created
/// * `position_mint` - New mint for position NFT
/// * `position_token_account` - Token account to hold position NFT
/// * `whirlpool` - The whirlpool to open position in
/// * `token_program` - SPL Token program
/// * `system_program` - System program
/// * `rent` - Rent sysvar
/// * `associated_token_program` - Associated Token program
/// * `tick_lower_index` - Lower tick boundary
/// * `tick_upper_index` - Upper tick boundary
/// * `position_bump` - Bump for position PDA
pub fn open_position<'info>(
    whirlpool_program: &AccountInfo<'info>,
    funder: &AccountInfo<'info>,
    owner: &AccountInfo<'info>,
    position: &AccountInfo<'info>,
    position_mint: &AccountInfo<'info>,
    position_token_account: &AccountInfo<'info>,
    whirlpool: &AccountInfo<'info>,
    token_program: &AccountInfo<'info>,
    system_program: &AccountInfo<'info>,
    rent: &AccountInfo<'info>,
    associated_token_program: &AccountInfo<'info>,
    tick_lower_index: i32,
    tick_upper_index: i32,
    position_bump: u8,
    signer_seeds: &[&[&[u8]]],
) -> Result<()> {
    let params = OpenPositionParams {
        bumps: OpenPositionBumps { position_bump },
        tick_lower_index,
        tick_upper_index,
    };

    let mut data = Vec::with_capacity(8 + 1 + 4 + 4);
    data.extend_from_slice(&instruction::OPEN_POSITION);
    params.serialize(&mut data)?;

    let accounts = vec![
        AccountMeta::new(*funder.key, true),
        AccountMeta::new_readonly(*owner.key, false),
        AccountMeta::new(*position.key, false),
        AccountMeta::new(*position_mint.key, true),
        AccountMeta::new(*position_token_account.key, false),
        AccountMeta::new_readonly(*whirlpool.key, false),
        AccountMeta::new_readonly(*token_program.key, false),
        AccountMeta::new_readonly(*system_program.key, false),
        AccountMeta::new_readonly(*rent.key, false),
        AccountMeta::new_readonly(*associated_token_program.key, false),
    ];

    let ix = anchor_lang::solana_program::instruction::Instruction {
        program_id: *whirlpool_program.key,
        accounts,
        data,
    };

    anchor_lang::solana_program::program::invoke_signed(
        &ix,
        &[
            funder.clone(),
            owner.clone(),
            position.clone(),
            position_mint.clone(),
            position_token_account.clone(),
            whirlpool.clone(),
            token_program.clone(),
            system_program.clone(),
            rent.clone(),
            associated_token_program.clone(),
        ],
        signer_seeds,
    )?;

    Ok(())
}

// =============================================================================
// INCREASE LIQUIDITY CPI
// =============================================================================

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct IncreaseLiquidityParams {
    pub liquidity_amount: u128,
    pub token_max_a: u64,
    pub token_max_b: u64,
}

/// Increase liquidity in an existing position.
/// 
/// # Arguments
/// * `whirlpool_program` - Whirlpool program account
/// * `whirlpool` - The whirlpool
/// * `token_program` - SPL Token program
/// * `position_authority` - Authority that can modify position (user or PDA)
/// * `position` - The position to add liquidity to
/// * `position_token_account` - Token account holding position NFT
/// * `token_owner_account_a` - Token A source account
/// * `token_owner_account_b` - Token B source account
/// * `token_vault_a` - Whirlpool token A vault
/// * `token_vault_b` - Whirlpool token B vault
/// * `tick_array_lower` - Tick array containing lower tick
/// * `tick_array_upper` - Tick array containing upper tick
/// * `liquidity_amount` - Amount of liquidity to add
/// * `token_max_a` - Maximum token A to deposit
/// * `token_max_b` - Maximum token B to deposit
pub fn increase_liquidity<'info>(
    whirlpool_program: &AccountInfo<'info>,
    whirlpool: &AccountInfo<'info>,
    token_program: &AccountInfo<'info>,
    position_authority: &AccountInfo<'info>,
    position: &AccountInfo<'info>,
    position_token_account: &AccountInfo<'info>,
    token_owner_account_a: &AccountInfo<'info>,
    token_owner_account_b: &AccountInfo<'info>,
    token_vault_a: &AccountInfo<'info>,
    token_vault_b: &AccountInfo<'info>,
    tick_array_lower: &AccountInfo<'info>,
    tick_array_upper: &AccountInfo<'info>,
    liquidity_amount: u128,
    token_max_a: u64,
    token_max_b: u64,
    signer_seeds: &[&[&[u8]]],
) -> Result<()> {
    let params = IncreaseLiquidityParams {
        liquidity_amount,
        token_max_a,
        token_max_b,
    };

    let mut data = Vec::with_capacity(8 + 16 + 8 + 8);
    data.extend_from_slice(&instruction::INCREASE_LIQUIDITY);
    params.serialize(&mut data)?;

    let accounts = vec![
        AccountMeta::new(*whirlpool.key, false),
        AccountMeta::new_readonly(*token_program.key, false),
        AccountMeta::new_readonly(*position_authority.key, true),
        AccountMeta::new(*position.key, false),
        AccountMeta::new_readonly(*position_token_account.key, false),
        AccountMeta::new(*token_owner_account_a.key, false),
        AccountMeta::new(*token_owner_account_b.key, false),
        AccountMeta::new(*token_vault_a.key, false),
        AccountMeta::new(*token_vault_b.key, false),
        AccountMeta::new(*tick_array_lower.key, false),
        AccountMeta::new(*tick_array_upper.key, false),
    ];

    let ix = anchor_lang::solana_program::instruction::Instruction {
        program_id: *whirlpool_program.key,
        accounts,
        data,
    };

    anchor_lang::solana_program::program::invoke_signed(
        &ix,
        &[
            whirlpool.clone(),
            token_program.clone(),
            position_authority.clone(),
            position.clone(),
            position_token_account.clone(),
            token_owner_account_a.clone(),
            token_owner_account_b.clone(),
            token_vault_a.clone(),
            token_vault_b.clone(),
            tick_array_lower.clone(),
            tick_array_upper.clone(),
        ],
        signer_seeds,
    )?;

    Ok(())
}

// =============================================================================
// DECREASE LIQUIDITY CPI
// =============================================================================

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct DecreaseLiquidityParams {
    pub liquidity_amount: u128,
    pub token_min_a: u64,
    pub token_min_b: u64,
}

/// Decrease liquidity from an existing position.
pub fn decrease_liquidity<'info>(
    whirlpool_program: &AccountInfo<'info>,
    whirlpool: &AccountInfo<'info>,
    token_program: &AccountInfo<'info>,
    position_authority: &AccountInfo<'info>,
    position: &AccountInfo<'info>,
    position_token_account: &AccountInfo<'info>,
    token_owner_account_a: &AccountInfo<'info>,
    token_owner_account_b: &AccountInfo<'info>,
    token_vault_a: &AccountInfo<'info>,
    token_vault_b: &AccountInfo<'info>,
    tick_array_lower: &AccountInfo<'info>,
    tick_array_upper: &AccountInfo<'info>,
    liquidity_amount: u128,
    token_min_a: u64,
    token_min_b: u64,
    signer_seeds: &[&[&[u8]]],
) -> Result<()> {
    let params = DecreaseLiquidityParams {
        liquidity_amount,
        token_min_a,
        token_min_b,
    };

    let mut data = Vec::with_capacity(8 + 16 + 8 + 8);
    data.extend_from_slice(&instruction::DECREASE_LIQUIDITY);
    params.serialize(&mut data)?;

    let accounts = vec![
        AccountMeta::new(*whirlpool.key, false),
        AccountMeta::new_readonly(*token_program.key, false),
        AccountMeta::new_readonly(*position_authority.key, true),
        AccountMeta::new(*position.key, false),
        AccountMeta::new_readonly(*position_token_account.key, false),
        AccountMeta::new(*token_owner_account_a.key, false),
        AccountMeta::new(*token_owner_account_b.key, false),
        AccountMeta::new(*token_vault_a.key, false),
        AccountMeta::new(*token_vault_b.key, false),
        AccountMeta::new(*tick_array_lower.key, false),
        AccountMeta::new(*tick_array_upper.key, false),
    ];

    let ix = anchor_lang::solana_program::instruction::Instruction {
        program_id: *whirlpool_program.key,
        accounts,
        data,
    };

    anchor_lang::solana_program::program::invoke_signed(
        &ix,
        &[
            whirlpool.clone(),
            token_program.clone(),
            position_authority.clone(),
            position.clone(),
            position_token_account.clone(),
            token_owner_account_a.clone(),
            token_owner_account_b.clone(),
            token_vault_a.clone(),
            token_vault_b.clone(),
            tick_array_lower.clone(),
            tick_array_upper.clone(),
        ],
        signer_seeds,
    )?;

    Ok(())
}

// =============================================================================
// UPDATE FEES AND REWARDS CPI
// =============================================================================

/// Update fee and reward accumulators for a position.
/// Must be called before collect_fees to ensure fee_owed values are current.
pub fn update_fees_and_rewards<'info>(
    whirlpool_program: &AccountInfo<'info>,
    whirlpool: &AccountInfo<'info>,
    position: &AccountInfo<'info>,
    tick_array_lower: &AccountInfo<'info>,
    tick_array_upper: &AccountInfo<'info>,
) -> Result<()> {
    let data = instruction::UPDATE_FEES_AND_REWARDS.to_vec();

    let accounts = vec![
        AccountMeta::new_readonly(*whirlpool.key, false),
        AccountMeta::new(*position.key, false),
        AccountMeta::new_readonly(*tick_array_lower.key, false),
        AccountMeta::new_readonly(*tick_array_upper.key, false),
    ];

    let ix = anchor_lang::solana_program::instruction::Instruction {
        program_id: *whirlpool_program.key,
        accounts,
        data,
    };

    anchor_lang::solana_program::program::invoke(
        &ix,
        &[
            whirlpool.clone(),
            position.clone(),
            tick_array_lower.clone(),
            tick_array_upper.clone(),
        ],
    )?;

    Ok(())
}

// =============================================================================
// COLLECT FEES CPI
// =============================================================================

/// Collect accumulated fees from a position.
pub fn collect_fees<'info>(
    whirlpool_program: &AccountInfo<'info>,
    whirlpool: &AccountInfo<'info>,
    position_authority: &AccountInfo<'info>,
    position: &AccountInfo<'info>,
    position_token_account: &AccountInfo<'info>,
    token_owner_account_a: &AccountInfo<'info>,
    token_owner_account_b: &AccountInfo<'info>,
    token_vault_a: &AccountInfo<'info>,
    token_vault_b: &AccountInfo<'info>,
    token_program: &AccountInfo<'info>,
    signer_seeds: &[&[&[u8]]],
) -> Result<()> {
    let data = instruction::COLLECT_FEES.to_vec();

    let accounts = vec![
        AccountMeta::new_readonly(*whirlpool.key, false),
        AccountMeta::new_readonly(*position_authority.key, true),
        AccountMeta::new(*position.key, false),
        AccountMeta::new_readonly(*position_token_account.key, false),
        AccountMeta::new(*token_owner_account_a.key, false),
        AccountMeta::new(*token_vault_a.key, false),
        AccountMeta::new(*token_owner_account_b.key, false),
        AccountMeta::new(*token_vault_b.key, false),
        AccountMeta::new_readonly(*token_program.key, false),
    ];

    let ix = anchor_lang::solana_program::instruction::Instruction {
        program_id: *whirlpool_program.key,
        accounts,
        data,
    };

    anchor_lang::solana_program::program::invoke_signed(
        &ix,
        &[
            whirlpool.clone(),
            position_authority.clone(),
            position.clone(),
            position_token_account.clone(),
            token_owner_account_a.clone(),
            token_vault_a.clone(),
            token_owner_account_b.clone(),
            token_vault_b.clone(),
            token_program.clone(),
        ],
        signer_seeds,
    )?;

    Ok(())
}

// =============================================================================
// CLOSE POSITION CPI
// =============================================================================

/// Close a position and reclaim rent.
pub fn close_position<'info>(
    whirlpool_program: &AccountInfo<'info>,
    position_authority: &AccountInfo<'info>,
    receiver: &AccountInfo<'info>,
    position: &AccountInfo<'info>,
    position_mint: &AccountInfo<'info>,
    position_token_account: &AccountInfo<'info>,
    token_program: &AccountInfo<'info>,
    signer_seeds: &[&[&[u8]]],
) -> Result<()> {
    let data = instruction::CLOSE_POSITION.to_vec();

    let accounts = vec![
        AccountMeta::new_readonly(*position_authority.key, true),
        AccountMeta::new(*receiver.key, false),
        AccountMeta::new(*position.key, false),
        AccountMeta::new(*position_mint.key, false),
        AccountMeta::new(*position_token_account.key, false),
        AccountMeta::new_readonly(*token_program.key, false),
    ];

    let ix = anchor_lang::solana_program::instruction::Instruction {
        program_id: *whirlpool_program.key,
        accounts,
        data,
    };

    anchor_lang::solana_program::program::invoke_signed(
        &ix,
        &[
            position_authority.clone(),
            receiver.clone(),
            position.clone(),
            position_mint.clone(),
            position_token_account.clone(),
            token_program.clone(),
        ],
        signer_seeds,
    )?;

    Ok(())
}

// =============================================================================
// HELPER FUNCTIONS
// =============================================================================

/// Derive position PDA address
pub fn get_position_address(position_mint: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(
        &[b"position", position_mint.as_ref()],
        &WHIRLPOOL_PROGRAM_ID,
    )
}

/// Orca Whirlpool tick array size — number of initialized ticks per array.
pub const TICK_ARRAY_SIZE: i32 = 88;

/// Compute the start_tick_index for the tick array containing `tick_index`.
///
/// Orca Whirlpool organizes ticks into arrays of TICK_ARRAY_SIZE (88) ticks.
/// Each array covers a range of `tick_spacing * TICK_ARRAY_SIZE` tick indices.
/// Uses floor division (toward -infinity) for correct handling of negative ticks.
pub fn get_start_tick_index(tick_index: i32, tick_spacing: u16) -> i32 {
    let ticks_in_array = TICK_ARRAY_SIZE * (tick_spacing as i32);
    let mut start = tick_index / ticks_in_array;
    // Rust integer division truncates toward zero; for negative indices
    // we need floor division (toward -infinity).
    if tick_index < 0 && tick_index % ticks_in_array != 0 {
        start -= 1;
    }
    start * ticks_in_array
}

/// Derive tick array PDA address
pub fn get_tick_array_address(whirlpool: &Pubkey, start_tick_index: i32) -> (Pubkey, u8) {
    Pubkey::find_program_address(
        &[
            b"tick_array",
            whirlpool.as_ref(),
            start_tick_index.to_string().as_bytes(),
        ],
        &WHIRLPOOL_PROGRAM_ID,
    )
}
