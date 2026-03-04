use anchor_lang::prelude::*;
use anchor_spl::{
    associated_token::AssociatedToken,
    token::{self, Burn, Mint, MintTo, Token, TokenAccount, Transfer},
};

pub mod constants;
pub mod ed25519;
pub mod errors;
pub mod events;
pub mod state;
pub mod whirlpool_cpi;

use constants::*;
use ed25519::*;
use errors::*;
use events::*;
use state::*;

declare_id!("Bk81YHvFinrSCs64W7MzobDhMJNXrUEqAU5YpAWcotua");

/// ============================================================================
/// LP BONDS EVOLUTION PROGRAM
/// ============================================================================
///
/// Handles bond evolution from Level 1 to Level 4:
/// - Verify source bond ownership via base program PDAs
/// - Burn source bond NFT
/// - Mint layer tokens
/// - Add liquidity to target level Orca Whirlpool
/// - Mint new upgraded bond NFT
///
/// Mirrors EVM LiquidityBondsEvolution contract architecture.
/// ============================================================================

#[program]
pub mod lp_bonds_evolution {
    use super::*;

    /// Initialize evolution configuration.
    pub fn initialize_evolution(
        ctx: Context<InitializeEvolution>,
        treasury: Pubkey,
        oracle_authority: Pubkey,
    ) -> Result<()> {
        let config = &mut ctx.accounts.evolution_config;
        config.admin = ctx.accounts.admin.key();
        config.treasury = treasury;
        config.oracle_authority = oracle_authority;
        config.is_paused = false;
        config.evolution_counter = 0;
        config.bump = ctx.bumps.evolution_config;

        emit!(EvolutionInitialized {
            admin: ctx.accounts.admin.key(),
            treasury,
            oracle_authority,
            timestamp: Clock::get()?.unix_timestamp,
        });

        Ok(())
    }

    /// Initialize layer token authority PDA.
    pub fn initialize_layer_authority(
        ctx: Context<InitializeLayerAuthority>,
    ) -> Result<()> {
        let authority = &mut ctx.accounts.layer_token_authority;
        authority.bump = ctx.bumps.layer_token_authority;
        Ok(())
    }

    /// Create a layer token mint controlled by the evolution program.
    /// The layer_token_authority PDA is set as the mint authority.
    pub fn create_layer_token_mint(
        ctx: Context<CreateLayerTokenMint>,
        decimals: u8,
    ) -> Result<()> {
        msg!("Creating layer token mint with {} decimals", decimals);
        msg!("Mint authority: {}", ctx.accounts.layer_token_authority.key());
        msg!("Layer token mint: {}", ctx.accounts.layer_token_mint.key());
        Ok(())
    }

    /// Configure a level for evolution.
    pub fn configure_level(
        ctx: Context<ConfigureLevel>,
        level_id: u8,
        tick_lower: i32,
        tick_upper: i32,
        required_amount_a: u64,
        required_amount_b: u64,
        fee_bps: u16,
        lock_duration: i64,
        multiplier: u16,
        is_active: bool,
    ) -> Result<()> {
        require!(level_id >= 2 && level_id <= MAX_BOND_LEVEL, EvolutionError::InvalidBondLevel);
        require!(fee_bps <= MAX_FEE_BPS, EvolutionError::FeeTooHigh);
        require!(tick_lower < tick_upper, EvolutionError::InvalidTickRange);

        let expected_whirlpool = get_whirlpool_for_level(level_id)
            .ok_or(EvolutionError::InvalidBondLevel)?;
        require_keys_eq!(
            ctx.accounts.whirlpool.key(),
            expected_whirlpool,
            EvolutionError::WhirlpoolLevelMismatch
        );

        let config = &mut ctx.accounts.level_config;
        config.level_id = level_id;
        config.whirlpool = ctx.accounts.whirlpool.key();
        config.token_mint_a = ctx.accounts.token_mint_a.key();
        config.token_mint_b = ctx.accounts.token_mint_b.key();
        config.layer_token_mint = ctx.accounts.layer_token_mint.key();
        config.tick_lower = tick_lower;
        config.tick_upper = tick_upper;
        config.required_amount_a = required_amount_a;
        config.required_amount_b = required_amount_b;
        config.fee_bps = fee_bps;
        config.lock_duration = lock_duration;
        config.multiplier = multiplier;
        config.is_active = is_active;
        config.bump = ctx.bumps.level_config;

        emit!(LevelConfigured {
            level_id,
            whirlpool: ctx.accounts.whirlpool.key(),
            token_mint_a: ctx.accounts.token_mint_a.key(),
            token_mint_b: ctx.accounts.token_mint_b.key(),
            layer_token_mint: ctx.accounts.layer_token_mint.key(),
            required_amount_a,
            required_amount_b,
            fee_bps,
            lock_duration,
            is_active,
            admin: ctx.accounts.admin.key(),
            timestamp: Clock::get()?.unix_timestamp,
        });

        Ok(())
    }

    /// Pause evolution.
    pub fn pause_evolution(ctx: Context<PauseEvolution>) -> Result<()> {
        ctx.accounts.evolution_config.is_paused = true;
        emit!(EvolutionPaused {
            admin: ctx.accounts.admin.key(),
            timestamp: Clock::get()?.unix_timestamp,
        });
        Ok(())
    }

    /// Unpause evolution.
    pub fn unpause_evolution(ctx: Context<UnpauseEvolution>) -> Result<()> {
        ctx.accounts.evolution_config.is_paused = false;
        emit!(EvolutionUnpaused {
            admin: ctx.accounts.admin.key(),
            timestamp: Clock::get()?.unix_timestamp,
        });
        Ok(())
    }

    /// Update treasury.
    pub fn update_treasury(
        ctx: Context<UpdateTreasury>,
        new_treasury: Pubkey,
    ) -> Result<()> {
        let config = &mut ctx.accounts.evolution_config;
        let old_treasury = config.treasury;
        config.treasury = new_treasury;

        emit!(EvolutionTreasuryUpdated {
            old_treasury,
            new_treasury,
            admin: ctx.accounts.admin.key(),
            timestamp: Clock::get()?.unix_timestamp,
        });

        Ok(())
    }

    /// Update oracle authority.
    pub fn update_oracle(
        ctx: Context<UpdateOracle>,
        new_oracle: Pubkey,
    ) -> Result<()> {
        let config = &mut ctx.accounts.evolution_config;
        let old_oracle = config.oracle_authority;
        config.oracle_authority = new_oracle;

        emit!(EvolutionOracleUpdated {
            old_oracle,
            new_oracle,
            admin: ctx.accounts.admin.key(),
            timestamp: Clock::get()?.unix_timestamp,
        });

        Ok(())
    }

    /// Evolve a bond to the next level.
    ///
    /// remaining_accounts[0] = tick_array_lower
    /// remaining_accounts[1] = tick_array_upper
    pub fn evolve_bond<'info>(
        ctx: Context<'_, '_, 'info, 'info, EvolveBond<'info>>,
        target_level: u8,
        amount_a: u64,
        amount_b: u64,
        liquidity_amount: u128,
        token_max_a: u64,
        token_max_b: u64,
        tick_lower_index: i32,
        tick_upper_index: i32,
        nonce: u64,
    ) -> Result<()> {
        // Extract tick arrays from remaining accounts
        require!(
            ctx.remaining_accounts.len() >= 2,
            EvolutionError::InsufficientRemainingAccounts
        );
        let tick_array_lower: &AccountInfo<'info> = &ctx.remaining_accounts[0];
        let tick_array_upper: &AccountInfo<'info> = &ctx.remaining_accounts[1];

        // Validate evolution not paused
        require!(
            !ctx.accounts.evolution_config.is_paused,
            EvolutionError::EvolutionPaused
        );

        // Deserialize source custody from base program
        let source_custody_data = ctx.accounts.source_custody.try_borrow_data()?;
        // Skip 8-byte discriminator, then read level at offset based on PositionCustody layout:
        // bond_mint(32) + position_mint(32) + whirlpool(32) + tick_lower(4) + tick_upper(4) + 
        // liquidity(16) + depositor(32) + created_at(8) = 160 bytes, then level(1)
        let raw_level = source_custody_data[8 + 160]; // 8 for discriminator + 160 offset to level field
        drop(source_custody_data);
        
        // Handle legacy bonds that may have uninitialized level field (0 or 255)
        // Treat them as Level 1 bonds
        let source_level: u8 = if raw_level == 0 || raw_level == 255 { 1 } else { raw_level };

        // Validate level transition
        require!(
            target_level == source_level.checked_add(1).ok_or(EvolutionError::MaxLevelReached)?,
            EvolutionError::InvalidLevelTransition
        );
        require!(target_level <= MAX_BOND_LEVEL, EvolutionError::MaxLevelReached);

        // Validate level config
        require!(ctx.accounts.level_config.is_active, EvolutionError::LevelNotActive);

        let expected_whirlpool = get_whirlpool_for_level(target_level)
            .ok_or(EvolutionError::InvalidBondLevel)?;
        require_keys_eq!(
            ctx.accounts.whirlpool.key(),
            expected_whirlpool,
            EvolutionError::WhirlpoolLevelMismatch
        );

        // Validate nonce (read from base program PDA)
        let nonce_data = ctx.accounts.nonce_account.try_borrow_data()?;
        let current_nonce = u64::from_le_bytes(
            nonce_data[40..48].try_into().unwrap()
        );
        drop(nonce_data);
        require!(nonce > current_nonce, EvolutionError::NonceAlreadyUsed);

        // Verify signature
        let evolution_params = EvolutionCanonicalMessageParams {
            source_bond_mint: ctx.accounts.source_bond_mint.key(),
            target_level,
            amount_a,
            amount_b,
            liquidity: liquidity_amount,
            nonce,
            sender: ctx.accounts.user.key(),
            contract_address: crate::ID,
        };

        verify_evolution_signature(
            &ctx.accounts.instructions_sysvar,
            &ctx.accounts.evolution_config.oracle_authority,
            &evolution_params,
        )?;

        // Verify user owns source bond
        require!(
            ctx.accounts.user_source_bond_account.amount == 1,
            EvolutionError::InvalidBondBalance
        );

        // STEP 1: Burn source bond NFT
        token::burn(
            CpiContext::new(
                ctx.accounts.token_program.to_account_info(),
                Burn {
                    mint: ctx.accounts.source_bond_mint.to_account_info(),
                    from: ctx.accounts.user_source_bond_account.to_account_info(),
                    authority: ctx.accounts.user.to_account_info(),
                },
            ),
            1,
        )?;

        // STEP 2: Transfer token A from user
        token::transfer(
            CpiContext::new(
                ctx.accounts.token_program.to_account_info(),
                Transfer {
                    from: ctx.accounts.user_token_a_account.to_account_info(),
                    to: ctx.accounts.program_token_a_account.to_account_info(),
                    authority: ctx.accounts.user.to_account_info(),
                },
            ),
            amount_a,
        )?;

        // STEP 3: Transfer token B from user (layer token / whirlpool Token B)
        token::transfer(
            CpiContext::new(
                ctx.accounts.token_program.to_account_info(),
                Transfer {
                    from: ctx.accounts.user_token_b_account.to_account_info(),
                    to: ctx.accounts.program_token_b_account.to_account_info(),
                    authority: ctx.accounts.user.to_account_info(),
                },
            ),
            amount_b,
        )?;

        // Define signer seeds for layer_token_authority
        let authority_seeds: &[&[u8]] = &[
            LAYER_TOKEN_AUTHORITY_SEED,
            &[ctx.accounts.layer_token_authority.bump],
        ];
        let signer_seeds = &[authority_seeds];

        // STEP 4: Deduct protocol fee
        let fee = ctx.accounts.level_config.calculate_fee(amount_a);
        if fee > 0 {
            token::transfer(
                CpiContext::new_with_signer(
                    ctx.accounts.token_program.to_account_info(),
                    Transfer {
                        from: ctx.accounts.program_token_a_account.to_account_info(),
                        to: ctx.accounts.treasury_token_account.to_account_info(),
                        authority: ctx.accounts.layer_token_authority.to_account_info(),
                    },
                    signer_seeds,
                ),
                fee,
            )?;
        }

        // STEP 5: Approve tokens for Whirlpool
        let deposit_amount_a = amount_a.saturating_sub(fee);

        token::approve(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                token::Approve {
                    to: ctx.accounts.program_token_a_account.to_account_info(),
                    delegate: ctx.accounts.whirlpool_program.to_account_info(),
                    authority: ctx.accounts.layer_token_authority.to_account_info(),
                },
                signer_seeds,
            ),
            deposit_amount_a,
        )?;

        token::approve(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                token::Approve {
                    to: ctx.accounts.program_token_b_account.to_account_info(),
                    delegate: ctx.accounts.whirlpool_program.to_account_info(),
                    authority: ctx.accounts.layer_token_authority.to_account_info(),
                },
                signer_seeds,
            ),
            amount_b,
        )?;

        // STEP 6: Open Whirlpool position
        let (position_pda, position_bump) = whirlpool_cpi::get_position_address(
            &ctx.accounts.position_mint.key(),
        );
        require_keys_eq!(
            ctx.accounts.whirlpool_position.key(),
            position_pda,
            EvolutionError::InvalidPositionPda
        );

        whirlpool_cpi::open_position(
            &ctx.accounts.whirlpool_program.to_account_info(),
            &ctx.accounts.user.to_account_info(),
            &ctx.accounts.user.to_account_info(),
            &ctx.accounts.whirlpool_position.to_account_info(),
            &ctx.accounts.position_mint.to_account_info(),
            &ctx.accounts.position_token_account.to_account_info(),
            &ctx.accounts.whirlpool.to_account_info(),
            &ctx.accounts.token_program.to_account_info(),
            &ctx.accounts.system_program.to_account_info(),
            &ctx.accounts.rent.to_account_info(),
            &ctx.accounts.associated_token_program.to_account_info(),
            tick_lower_index,
            tick_upper_index,
            position_bump,
            &[],
        )?;

        // STEP 7: Create custody position token account
        anchor_spl::associated_token::create(CpiContext::new(
            ctx.accounts.associated_token_program.to_account_info(),
            anchor_spl::associated_token::Create {
                payer: ctx.accounts.user.to_account_info(),
                associated_token: ctx.accounts.custody_position_token_account.to_account_info(),
                authority: ctx.accounts.layer_token_authority.to_account_info(),
                mint: ctx.accounts.position_mint.to_account_info(),
                system_program: ctx.accounts.system_program.to_account_info(),
                token_program: ctx.accounts.token_program.to_account_info(),
            },
        ))?;

        // STEP 7.5: Transfer position NFT to custody for increase_liquidity
        // The position_authority must own the position NFT and the token accounts
        // Transfer NFT from user's position_token_account to custody_position_token_account
        token::transfer(
            CpiContext::new(
                ctx.accounts.token_program.to_account_info(),
                Transfer {
                    from: ctx.accounts.position_token_account.to_account_info(),
                    to: ctx.accounts.custody_position_token_account.to_account_info(),
                    authority: ctx.accounts.user.to_account_info(),
                },
            ),
            1,
        )?;

        // STEP 8: Add liquidity using layer_token_authority as position authority
        // The custody_position_token_account is an ATA owned by position_custody PDA
        // We use layer_token_authority as position_authority since it owns the token accounts
        whirlpool_cpi::increase_liquidity(
            &ctx.accounts.whirlpool_program.to_account_info(),
            &ctx.accounts.whirlpool.to_account_info(),
            &ctx.accounts.token_program.to_account_info(),
            &ctx.accounts.layer_token_authority.to_account_info(),
            &ctx.accounts.whirlpool_position.to_account_info(),
            &ctx.accounts.custody_position_token_account.to_account_info(),
            &ctx.accounts.program_token_a_account.to_account_info(),
            &ctx.accounts.program_token_b_account.to_account_info(),
            &ctx.accounts.token_vault_a.to_account_info(),
            &ctx.accounts.token_vault_b.to_account_info(),
            tick_array_lower,
            tick_array_upper,
            liquidity_amount,
            token_max_a,
            token_max_b,
            signer_seeds,
        )?;

        // STEP 9: Mint new bond NFT
        let bond_authority_seeds: &[&[u8]] = &[
            BOND_AUTHORITY_SEED,
            &[ctx.bumps.bond_authority],
        ];
        let bond_signer_seeds = &[bond_authority_seeds];

        token::mint_to(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                MintTo {
                    mint: ctx.accounts.target_bond_mint.to_account_info(),
                    to: ctx.accounts.user_target_bond_account.to_account_info(),
                    authority: ctx.accounts.bond_authority.to_account_info(),
                },
                bond_signer_seeds,
            ),
            1,
        )?;

        // STEP 11: Initialize position custody
        let custody = &mut ctx.accounts.position_custody;
        custody.bond_mint = ctx.accounts.target_bond_mint.key();
        custody.position_mint = ctx.accounts.position_mint.key();
        custody.whirlpool = ctx.accounts.whirlpool.key();
        custody.tick_lower_index = tick_lower_index;
        custody.tick_upper_index = tick_upper_index;
        custody.liquidity = liquidity_amount;
        custody.depositor = ctx.accounts.user.key();
        custody.created_at = Clock::get()?.unix_timestamp;
        custody.level = target_level;
        custody.lock_duration = ctx.accounts.level_config.lock_duration;
        custody.is_evolved = true;
        custody.evolved_from = ctx.accounts.source_bond_mint.key();
        custody.bump = ctx.bumps.position_custody;
        custody.position_bump = position_bump;

        // STEP 12: Create evolution record
        let record = &mut ctx.accounts.evolution_record;
        record.source_bond_mint = ctx.accounts.source_bond_mint.key();
        record.source_level = source_level;
        record.target_bond_mint = ctx.accounts.target_bond_mint.key();
        record.target_level = target_level;
        record.evolver = ctx.accounts.user.key();
        record.evolved_at = Clock::get()?.unix_timestamp;
        record.amount_a = amount_a;
        record.amount_b = amount_b;
        record.liquidity = liquidity_amount;
        record.fee_paid = fee;
        record.bump = ctx.bumps.evolution_record;

        // Update counter
        let config = &mut ctx.accounts.evolution_config;
        config.evolution_counter = config.evolution_counter.checked_add(1).unwrap();

        emit!(BondEvolved {
            source_bond_mint: ctx.accounts.source_bond_mint.key(),
            source_level,
            target_bond_mint: ctx.accounts.target_bond_mint.key(),
            target_level,
            whirlpool: ctx.accounts.whirlpool.key(),
            position_mint: ctx.accounts.position_mint.key(),
            evolver: ctx.accounts.user.key(),
            amount_a,
            amount_b,
            liquidity: liquidity_amount,
            fee_paid: fee,
            lock_duration: ctx.accounts.level_config.lock_duration,
            timestamp: Clock::get()?.unix_timestamp,
        });

        msg!("Bond evolved: Level {} -> Level {}", source_level, target_level);

        Ok(())
    }
}

// =============================================================================
// ACCOUNT STRUCTS
// =============================================================================

#[derive(Accounts)]
pub struct InitializeEvolution<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,

    #[account(
        init,
        payer = admin,
        space = 8 + EvolutionConfig::INIT_SPACE,
        seeds = [EVOLUTION_CONFIG_SEED],
        bump,
    )]
    pub evolution_config: Account<'info, EvolutionConfig>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct InitializeLayerAuthority<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,

    #[account(
        seeds = [EVOLUTION_CONFIG_SEED],
        bump = evolution_config.bump,
        constraint = admin.key() == evolution_config.admin @ EvolutionError::InvalidAdminAuthority,
    )]
    pub evolution_config: Account<'info, EvolutionConfig>,

    #[account(
        init,
        payer = admin,
        space = 8 + LayerTokenAuthority::INIT_SPACE,
        seeds = [LAYER_TOKEN_AUTHORITY_SEED],
        bump,
    )]
    pub layer_token_authority: Account<'info, LayerTokenAuthority>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(decimals: u8)]
pub struct CreateLayerTokenMint<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,

    #[account(
        seeds = [EVOLUTION_CONFIG_SEED],
        bump = evolution_config.bump,
        constraint = admin.key() == evolution_config.admin @ EvolutionError::InvalidAdminAuthority,
    )]
    pub evolution_config: Account<'info, EvolutionConfig>,

    #[account(seeds = [LAYER_TOKEN_AUTHORITY_SEED], bump = layer_token_authority.bump)]
    pub layer_token_authority: Account<'info, LayerTokenAuthority>,

    #[account(
        init,
        payer = admin,
        mint::decimals = decimals,
        mint::authority = layer_token_authority,
        mint::freeze_authority = layer_token_authority,
    )]
    pub layer_token_mint: Account<'info, Mint>,

    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(level_id: u8)]
pub struct ConfigureLevel<'info> {
    #[account(
        mut,
        constraint = admin.key() == evolution_config.admin @ EvolutionError::InvalidAdminAuthority,
    )]
    pub admin: Signer<'info>,

    #[account(mut, seeds = [EVOLUTION_CONFIG_SEED], bump = evolution_config.bump)]
    pub evolution_config: Account<'info, EvolutionConfig>,

    #[account(
        init_if_needed,
        payer = admin,
        space = 8 + LevelConfig::INIT_SPACE,
        seeds = [LEVEL_CONFIG_SEED, &[level_id]],
        bump,
    )]
    pub level_config: Account<'info, LevelConfig>,

    /// CHECK: Validated against hardcoded constants
    pub whirlpool: UncheckedAccount<'info>,

    pub token_mint_a: Account<'info, Mint>,
    pub token_mint_b: Account<'info, Mint>,
    pub layer_token_mint: Account<'info, Mint>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct PauseEvolution<'info> {
    #[account(constraint = admin.key() == evolution_config.admin @ EvolutionError::InvalidAdminAuthority)]
    pub admin: Signer<'info>,

    #[account(mut, seeds = [EVOLUTION_CONFIG_SEED], bump = evolution_config.bump)]
    pub evolution_config: Account<'info, EvolutionConfig>,
}

#[derive(Accounts)]
pub struct UnpauseEvolution<'info> {
    #[account(constraint = admin.key() == evolution_config.admin @ EvolutionError::InvalidAdminAuthority)]
    pub admin: Signer<'info>,

    #[account(mut, seeds = [EVOLUTION_CONFIG_SEED], bump = evolution_config.bump)]
    pub evolution_config: Account<'info, EvolutionConfig>,
}

#[derive(Accounts)]
pub struct UpdateTreasury<'info> {
    #[account(constraint = admin.key() == evolution_config.admin @ EvolutionError::InvalidAdminAuthority)]
    pub admin: Signer<'info>,

    #[account(mut, seeds = [EVOLUTION_CONFIG_SEED], bump = evolution_config.bump)]
    pub evolution_config: Account<'info, EvolutionConfig>,
}

#[derive(Accounts)]
pub struct UpdateOracle<'info> {
    #[account(constraint = admin.key() == evolution_config.admin @ EvolutionError::InvalidAdminAuthority)]
    pub admin: Signer<'info>,

    #[account(mut, seeds = [EVOLUTION_CONFIG_SEED], bump = evolution_config.bump)]
    pub evolution_config: Account<'info, EvolutionConfig>,
}

/// Position custody for the evolution program.
/// Note: This mirrors the base program structure for new bonds.
#[account]
#[derive(InitSpace)]
pub struct PositionCustody {
    pub bond_mint: Pubkey,
    pub position_mint: Pubkey,
    pub whirlpool: Pubkey,
    pub tick_lower_index: i32,
    pub tick_upper_index: i32,
    pub liquidity: u128,
    pub depositor: Pubkey,
    pub created_at: i64,
    pub level: u8,
    pub lock_duration: i64,
    pub is_evolved: bool,
    pub evolved_from: Pubkey,
    pub bump: u8,
    pub position_bump: u8,
}

#[derive(Accounts)]
#[instruction(
    target_level: u8,
    amount_a: u64,
    amount_b: u64,
    liquidity_amount: u128,
    token_max_a: u64,
    token_max_b: u64,
    tick_lower_index: i32,
    tick_upper_index: i32,
    nonce: u64,
)]
pub struct EvolveBond<'info> {
    #[account(mut)]
    pub user: Signer<'info>,

    #[account(mut, seeds = [EVOLUTION_CONFIG_SEED], bump = evolution_config.bump)]
    pub evolution_config: Box<Account<'info, EvolutionConfig>>,

    #[account(seeds = [LEVEL_CONFIG_SEED, &[target_level]], bump = level_config.bump)]
    pub level_config: Box<Account<'info, LevelConfig>>,

    /// CHECK: Nonce account from base lp-bonds program, validated by reading data
    pub nonce_account: UncheckedAccount<'info>,

    #[account(mut)]
    pub source_bond_mint: Box<Account<'info, Mint>>,

    #[account(
        mut,
        constraint = user_source_bond_account.owner == user.key() @ EvolutionError::InvalidTokenOwner,
        constraint = user_source_bond_account.mint == source_bond_mint.key() @ EvolutionError::InvalidBondMint,
    )]
    pub user_source_bond_account: Box<Account<'info, TokenAccount>>,

    /// CHECK: Source custody from base lp-bonds program OR evolution program (for multi-level evolutions).
    #[account(
        constraint = (source_custody.owner == &LP_BONDS_PROGRAM_ID || source_custody.owner == &crate::ID) @ EvolutionError::InvalidCustodyPda,
    )]
    pub source_custody: UncheckedAccount<'info>,

    /// CHECK: PDA for bond minting
    #[account(seeds = [BOND_AUTHORITY_SEED], bump)]
    pub bond_authority: UncheckedAccount<'info>,

    #[account(
        init,
        payer = user,
        mint::decimals = 0,
        mint::authority = bond_authority,
        mint::freeze_authority = bond_authority,
    )]
    pub target_bond_mint: Box<Account<'info, Mint>>,

    #[account(
        init_if_needed,
        payer = user,
        associated_token::mint = target_bond_mint,
        associated_token::authority = user,
    )]
    pub user_target_bond_account: Box<Account<'info, TokenAccount>>,

    #[account(
        init,
        payer = user,
        space = 8 + PositionCustody::INIT_SPACE,
        seeds = [POSITION_CUSTODY_SEED, target_bond_mint.key().as_ref()],
        bump,
    )]
    pub position_custody: Box<Account<'info, PositionCustody>>,

    #[account(
        init,
        payer = user,
        space = 8 + EvolutionRecord::INIT_SPACE,
        seeds = [EVOLUTION_RECORD_SEED, source_bond_mint.key().as_ref()],
        bump,
    )]
    pub evolution_record: Box<Account<'info, EvolutionRecord>>,

    #[account(constraint = token_mint_a.key() == level_config.token_mint_a @ EvolutionError::InvalidTokenMint)]
    pub token_mint_a: Box<Account<'info, Mint>>,

    #[account(
        mut,
        constraint = layer_token_mint.key() == level_config.layer_token_mint @ EvolutionError::InvalidTokenMint,
    )]
    pub layer_token_mint: Box<Account<'info, Mint>>,

    #[account(seeds = [LAYER_TOKEN_AUTHORITY_SEED], bump = layer_token_authority.bump)]
    pub layer_token_authority: Account<'info, LayerTokenAuthority>,

    #[account(
        mut,
        constraint = user_token_a_account.owner == user.key() @ EvolutionError::InvalidTokenOwner,
        constraint = user_token_a_account.mint == token_mint_a.key() @ EvolutionError::InvalidTokenMint,
    )]
    pub user_token_a_account: Box<Account<'info, TokenAccount>>,

    #[account(
        mut,
        constraint = user_token_b_account.owner == user.key() @ EvolutionError::InvalidTokenOwner,
        constraint = user_token_b_account.mint == layer_token_mint.key() @ EvolutionError::InvalidTokenMint,
    )]
    pub user_token_b_account: Box<Account<'info, TokenAccount>>,

    #[account(
        init_if_needed,
        payer = user,
        associated_token::mint = token_mint_a,
        associated_token::authority = layer_token_authority,
    )]
    pub program_token_a_account: Box<Account<'info, TokenAccount>>,

    #[account(
        init_if_needed,
        payer = user,
        associated_token::mint = layer_token_mint,
        associated_token::authority = layer_token_authority,
    )]
    pub program_token_b_account: Box<Account<'info, TokenAccount>>,

    #[account(
        mut,
        constraint = treasury_token_account.owner == evolution_config.treasury @ EvolutionError::InvalidTokenOwner,
    )]
    pub treasury_token_account: Box<Account<'info, TokenAccount>>,

    /// CHECK: Validated against level config
    #[account(
        mut,
        constraint = whirlpool.key() == level_config.whirlpool @ EvolutionError::WhirlpoolLevelMismatch,
    )]
    pub whirlpool: UncheckedAccount<'info>,

    /// CHECK: Validated by Orca Whirlpool
    #[account(mut)]
    pub token_vault_a: UncheckedAccount<'info>,

    /// CHECK: Validated by Orca Whirlpool
    #[account(mut)]
    pub token_vault_b: UncheckedAccount<'info>,

    /// CHECK: Initialized by Orca Whirlpool CPI
    #[account(mut)]
    pub position_mint: Signer<'info>,

    /// CHECK: Created by Orca Whirlpool CPI
    #[account(mut)]
    pub whirlpool_position: UncheckedAccount<'info>,

    /// CHECK: Created by Orca Whirlpool CPI
    #[account(mut)]
    pub position_token_account: UncheckedAccount<'info>,

    /// CHECK: Created after open_position
    #[account(mut)]
    pub custody_position_token_account: UncheckedAccount<'info>,

    /// CHECK: Orca Whirlpool program
    #[account(address = whirlpool_cpi::WHIRLPOOL_PROGRAM_ID @ EvolutionError::InvalidWhirlpoolProgram)]
    pub whirlpool_program: UncheckedAccount<'info>,

    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
    pub rent: Sysvar<'info, Rent>,

    /// CHECK: Instructions sysvar
    #[account(address = anchor_lang::solana_program::sysvar::instructions::ID)]
    pub instructions_sysvar: AccountInfo<'info>,
}

/// Reference to source custody from base program.
#[account]
#[derive(InitSpace)]
pub struct SourceCustodyRef {
    pub bond_mint: Pubkey,
    pub position_mint: Pubkey,
    pub whirlpool: Pubkey,
    pub tick_lower_index: i32,
    pub tick_upper_index: i32,
    pub liquidity: u128,
    pub depositor: Pubkey,
    pub created_at: i64,
    pub level: u8,
    pub lock_duration: i64,
    pub is_evolved: bool,
    pub evolved_from: Pubkey,
    pub bump: u8,
    pub position_bump: u8,
}
