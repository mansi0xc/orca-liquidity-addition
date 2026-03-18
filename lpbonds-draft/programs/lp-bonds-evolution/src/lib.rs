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

declare_id!("9VAsVsZpSqkwT3jBXe9yqKd1GSy9pH4ZpDduttsGoXPr");

/// ============================================================================
/// LP BONDS EVOLUTION PROGRAM
/// ============================================================================
///
/// Handles bond evolution from Level 1 to Level 4:
/// - Verify source bond ownership via base program PDAs
/// - Burn source bond NFT
/// - Transfer tokens and add liquidity to target level Orca Whirlpool
/// - Mint new upgraded bond NFT
///
/// All whirlpool addresses, token mints, and level parameters are configurable
/// via LevelConfig accounts -- no hardcoded addresses.
///
/// Mirrors EVM LiquidityBondsEvolution contract architecture.
/// ============================================================================

#[program]
pub mod lp_bonds_evolution {
    use super::*;

    // =========================================================================
    // INITIALIZATION
    // =========================================================================

    /// Initialize evolution configuration.
    pub fn initialize_evolution(
        ctx: Context<InitializeEvolution>,
        treasury: Pubkey,
        oracle_authority: Pubkey,
        lp_bonds_program_id: Pubkey,
    ) -> Result<()> {
        let config = &mut ctx.accounts.evolution_config;
        config.admin = ctx.accounts.admin.key();
        config.pending_admin = Pubkey::default();
        config.treasury = treasury;
        config.oracle_authority = oracle_authority;
        config.lp_bonds_program_id = lp_bonds_program_id;
        config.is_paused = false;
        config.evolution_counter = 0;
        config.bump = ctx.bumps.evolution_config;

        emit!(EvolutionInitialized {
            admin: ctx.accounts.admin.key(),
            treasury,
            oracle_authority,
            lp_bonds_program_id,
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
    pub fn create_layer_token_mint(
        ctx: Context<CreateLayerTokenMint>,
        _decimals: u8,
    ) -> Result<()> {
        msg!("Layer token mint created: {}", ctx.accounts.layer_token_mint.key());
        Ok(())
    }

    // =========================================================================
    // ADMIN INSTRUCTIONS
    // =========================================================================

    /// Configure a level for evolution. Whirlpool must be owned by Orca.
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
        emit!(EvolutionPausedEvent {
            admin: ctx.accounts.admin.key(),
            timestamp: Clock::get()?.unix_timestamp,
        });
        Ok(())
    }

    /// Unpause evolution.
    pub fn unpause_evolution(ctx: Context<UnpauseEvolution>) -> Result<()> {
        ctx.accounts.evolution_config.is_paused = false;
        emit!(EvolutionUnpausedEvent {
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

    /// Propose admin transfer (two-step pattern).
    pub fn propose_admin(ctx: Context<ProposeAdmin>, new_admin: Pubkey) -> Result<()> {
        let config = &mut ctx.accounts.evolution_config;
        config.pending_admin = new_admin;

        emit!(AdminTransferProposed {
            current_admin: ctx.accounts.admin.key(),
            pending_admin: new_admin,
            timestamp: Clock::get()?.unix_timestamp,
        });

        Ok(())
    }

    /// Accept admin transfer. Must be called by the pending admin.
    pub fn accept_admin(ctx: Context<AcceptAdmin>) -> Result<()> {
        let config = &mut ctx.accounts.evolution_config;
        let old_admin = config.admin;
        config.admin = ctx.accounts.new_admin.key();
        config.pending_admin = Pubkey::default();

        emit!(AdminTransferAccepted {
            old_admin,
            new_admin: ctx.accounts.new_admin.key(),
            timestamp: Clock::get()?.unix_timestamp,
        });

        Ok(())
    }

    /// Add a whitelisted authority with specific permissions.
    pub fn add_authority(
        ctx: Context<AddAuthority>,
        permissions: u8,
    ) -> Result<()> {
        let whitelist = &mut ctx.accounts.authority_whitelist;
        whitelist.authority = ctx.accounts.authority.key();
        whitelist.permissions = permissions;
        whitelist.added_by = ctx.accounts.admin.key();
        whitelist.bump = ctx.bumps.authority_whitelist;

        emit!(AuthorityAdded {
            authority: ctx.accounts.authority.key(),
            permissions,
            added_by: ctx.accounts.admin.key(),
            timestamp: Clock::get()?.unix_timestamp,
        });

        Ok(())
    }

    /// Remove a whitelisted authority.
    pub fn remove_authority(ctx: Context<RemoveAuthority>) -> Result<()> {
        emit!(AuthorityRemoved {
            authority: ctx.accounts.authority_whitelist.authority,
            removed_by: ctx.accounts.admin.key(),
            timestamp: Clock::get()?.unix_timestamp,
        });

        Ok(())
    }

    /// Initialize evolution nonce for a user.
    pub fn initialize_evolution_nonce(ctx: Context<InitializeEvolutionNonce>) -> Result<()> {
        let nonce = &mut ctx.accounts.evolution_nonce;
        nonce.user = ctx.accounts.user.key();
        nonce.current_nonce = 0;
        nonce.bump = ctx.bumps.evolution_nonce;

        emit!(EvolutionNonceInitialized {
            user: ctx.accounts.user.key(),
            timestamp: Clock::get()?.unix_timestamp,
        });

        Ok(())
    }

    // =========================================================================
    // EVOLUTION
    // =========================================================================

    /// Evolve a bond to the next level.
    ///
    /// remaining_accounts[0] = tick_array_lower
    /// remaining_accounts[1] = tick_array_upper
    /// remaining_accounts[2] = token_vault_a (validated against whirlpool data)
    /// remaining_accounts[3] = token_vault_b (validated against whirlpool data)
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
        require!(
            ctx.remaining_accounts.len() >= 4,
            EvolutionError::InsufficientRemainingAccounts
        );
        let tick_array_lower: &AccountInfo<'info> = &ctx.remaining_accounts[0];
        let tick_array_upper: &AccountInfo<'info> = &ctx.remaining_accounts[1];
        let token_vault_a: &AccountInfo<'info> = &ctx.remaining_accounts[2];
        let token_vault_b: &AccountInfo<'info> = &ctx.remaining_accounts[3];

        require!(
            !ctx.accounts.evolution_config.is_paused,
            EvolutionError::EvolutionPaused
        );

        let now = Clock::get()?.unix_timestamp;

        // --- SOURCE CUSTODY VALIDATION ---
        // Validate source_custody is the correct PDA for source_bond_mint
        let source_custody_owner = *ctx.accounts.source_custody.owner;
        let lp_bonds_id = ctx.accounts.evolution_config.lp_bonds_program_id;
        require!(
            source_custody_owner == lp_bonds_id || source_custody_owner == crate::ID,
            EvolutionError::InvalidCustodyPda
        );

        let (expected_custody_pda, _) = Pubkey::find_program_address(
            &[b"position_custody", ctx.accounts.source_bond_mint.key().as_ref()],
            &source_custody_owner,
        );
        require_keys_eq!(
            ctx.accounts.source_custody.key(),
            expected_custody_pda,
            EvolutionError::InvalidCustodyPda
        );

        // Read and validate source custody data
        let source_custody_data = ctx.accounts.source_custody.try_borrow_data()?;
        require!(source_custody_data.len() >= 8 + 161, EvolutionError::InvalidAccountData);

        // Validate bond_mint field matches source_bond_mint (bytes 8..40)
        let custody_bond_mint = Pubkey::try_from(&source_custody_data[8..40])
            .map_err(|_| EvolutionError::InvalidAccountData)?;
        require_keys_eq!(
            custody_bond_mint,
            ctx.accounts.source_bond_mint.key(),
            EvolutionError::InvalidBondMint
        );

        // Read level at offset 8 + 160
        let raw_level = source_custody_data[8 + 160];
        drop(source_custody_data);

        let source_level: u8 = if raw_level == 0 || raw_level == 255 { 1 } else { raw_level };

        // --- LEVEL TRANSITION VALIDATION ---
        require!(
            target_level == source_level.checked_add(1).ok_or(EvolutionError::MaxLevelReached)?,
            EvolutionError::InvalidLevelTransition
        );
        require!(target_level <= MAX_BOND_LEVEL, EvolutionError::MaxLevelReached);
        require!(ctx.accounts.level_config.is_active, EvolutionError::LevelNotActive);

        // --- WHIRLPOOL VALIDATION ---
        // Validate whirlpool is owned by Orca Whirlpool program
        require!(
            *ctx.accounts.whirlpool.owner == whirlpool_cpi::WHIRLPOOL_PROGRAM_ID,
            EvolutionError::InvalidWhirlpoolProgram
        );

        // Validate token vaults against whirlpool on-chain data
        {
            let wp_data = ctx.accounts.whirlpool.try_borrow_data()?;
            require!(wp_data.len() >= 245, EvolutionError::InvalidAccountData);
            let vault_a = Pubkey::try_from(&wp_data[133..165])
                .map_err(|_| EvolutionError::InvalidAccountData)?;
            let vault_b = Pubkey::try_from(&wp_data[213..245])
                .map_err(|_| EvolutionError::InvalidAccountData)?;
            require_keys_eq!(token_vault_a.key(), vault_a, EvolutionError::InvalidTokenVault);
            require_keys_eq!(token_vault_b.key(), vault_b, EvolutionError::InvalidTokenVault);
        }

        // --- NONCE VALIDATION (own nonce, writable) ---
        let current_nonce = ctx.accounts.evolution_nonce.current_nonce;
        require!(nonce > current_nonce, EvolutionError::NonceAlreadyUsed);

        // --- SIGNATURE VERIFICATION ---
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

        // --- BOND OWNERSHIP ---
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

        // Pre-compute signer seeds (needed for mint and subsequent operations)
        let authority_seeds: &[&[u8]] = &[
            LAYER_TOKEN_AUTHORITY_SEED,
            &[ctx.accounts.layer_token_authority.bump],
        ];
        let signer_seeds = &[authority_seeds];

        // STEP 3: Mint layer tokens to program (EVM parity: curToken1.mint(address(this), ...))
        token::mint_to(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                MintTo {
                    mint: ctx.accounts.layer_token_mint.to_account_info(),
                    to: ctx.accounts.program_token_b_account.to_account_info(),
                    authority: ctx.accounts.layer_token_authority.to_account_info(),
                },
                signer_seeds,
            ),
            amount_b,
        )?;

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

        // STEP 8: Add liquidity
        whirlpool_cpi::increase_liquidity(
            &ctx.accounts.whirlpool_program.to_account_info(),
            &ctx.accounts.whirlpool.to_account_info(),
            &ctx.accounts.token_program.to_account_info(),
            &ctx.accounts.layer_token_authority.to_account_info(),
            &ctx.accounts.whirlpool_position.to_account_info(),
            &ctx.accounts.custody_position_token_account.to_account_info(),
            &ctx.accounts.program_token_a_account.to_account_info(),
            &ctx.accounts.program_token_b_account.to_account_info(),
            token_vault_a,
            token_vault_b,
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

        // STEP 10: Update nonce
        let old_nonce = ctx.accounts.evolution_nonce.current_nonce;
        ctx.accounts.evolution_nonce.current_nonce = nonce;

        emit!(EvolutionNonceIncremented {
            user: ctx.accounts.user.key(),
            old_nonce,
            new_nonce: nonce,
            timestamp: now,
        });

        // STEP 11: Initialize position custody
        let custody = &mut ctx.accounts.position_custody;
        custody.bond_mint = ctx.accounts.target_bond_mint.key();
        custody.position_mint = ctx.accounts.position_mint.key();
        custody.whirlpool = ctx.accounts.whirlpool.key();
        custody.tick_lower_index = tick_lower_index;
        custody.tick_upper_index = tick_upper_index;
        custody.liquidity = liquidity_amount;
        custody.depositor = ctx.accounts.user.key();
        custody.created_at = now;
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
        record.evolved_at = now;
        record.amount_a = amount_a;
        record.amount_b = amount_b;
        record.liquidity = liquidity_amount;
        record.fee_paid = fee;
        record.bump = ctx.bumps.evolution_record;

        // Update counter
        let config = &mut ctx.accounts.evolution_config;
        config.evolution_counter = config.evolution_counter
            .checked_add(1)
            .ok_or(EvolutionError::ArithmeticOverflow)?;

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
            timestamp: now,
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

    /// CHECK: Validated to be owned by Orca Whirlpool program
    #[account(
        constraint = *whirlpool.owner == whirlpool_cpi::WHIRLPOOL_PROGRAM_ID @ EvolutionError::InvalidWhirlpoolProgram,
    )]
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

#[derive(Accounts)]
pub struct ProposeAdmin<'info> {
    #[account(constraint = admin.key() == evolution_config.admin @ EvolutionError::InvalidAdminAuthority)]
    pub admin: Signer<'info>,

    #[account(mut, seeds = [EVOLUTION_CONFIG_SEED], bump = evolution_config.bump)]
    pub evolution_config: Account<'info, EvolutionConfig>,
}

#[derive(Accounts)]
pub struct AcceptAdmin<'info> {
    #[account(
        constraint = new_admin.key() == evolution_config.pending_admin @ EvolutionError::InvalidPendingAdmin,
        constraint = evolution_config.pending_admin != Pubkey::default() @ EvolutionError::NoPendingAdmin,
    )]
    pub new_admin: Signer<'info>,

    #[account(mut, seeds = [EVOLUTION_CONFIG_SEED], bump = evolution_config.bump)]
    pub evolution_config: Account<'info, EvolutionConfig>,
}

#[derive(Accounts)]
pub struct AddAuthority<'info> {
    #[account(
        mut,
        constraint = admin.key() == evolution_config.admin @ EvolutionError::InvalidAdminAuthority,
    )]
    pub admin: Signer<'info>,

    #[account(seeds = [EVOLUTION_CONFIG_SEED], bump = evolution_config.bump)]
    pub evolution_config: Account<'info, EvolutionConfig>,

    /// CHECK: The authority being whitelisted
    pub authority: UncheckedAccount<'info>,

    #[account(
        init,
        payer = admin,
        space = 8 + AuthorityWhitelist::INIT_SPACE,
        seeds = [AUTHORITY_WHITELIST_SEED, authority.key().as_ref()],
        bump,
    )]
    pub authority_whitelist: Account<'info, AuthorityWhitelist>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct RemoveAuthority<'info> {
    #[account(
        mut,
        constraint = admin.key() == evolution_config.admin @ EvolutionError::InvalidAdminAuthority,
    )]
    pub admin: Signer<'info>,

    #[account(seeds = [EVOLUTION_CONFIG_SEED], bump = evolution_config.bump)]
    pub evolution_config: Account<'info, EvolutionConfig>,

    #[account(
        mut,
        close = admin,
        seeds = [AUTHORITY_WHITELIST_SEED, authority_whitelist.authority.as_ref()],
        bump = authority_whitelist.bump,
    )]
    pub authority_whitelist: Account<'info, AuthorityWhitelist>,
}

#[derive(Accounts)]
pub struct InitializeEvolutionNonce<'info> {
    #[account(mut)]
    pub user: Signer<'info>,

    #[account(
        init,
        payer = user,
        space = 8 + EvolutionNonce::INIT_SPACE,
        seeds = [EVOLUTION_NONCE_SEED, user.key().as_ref()],
        bump,
    )]
    pub evolution_nonce: Account<'info, EvolutionNonce>,

    pub system_program: Program<'info, System>,
}

/// Position custody for the evolution program.
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

    #[account(
        mut,
        seeds = [EVOLUTION_NONCE_SEED, user.key().as_ref()],
        bump = evolution_nonce.bump,
        constraint = evolution_nonce.user == user.key() @ EvolutionError::InvalidTokenOwner,
    )]
    pub evolution_nonce: Box<Account<'info, EvolutionNonce>>,

    #[account(mut)]
    pub source_bond_mint: Box<Account<'info, Mint>>,

    #[account(
        mut,
        constraint = user_source_bond_account.owner == user.key() @ EvolutionError::InvalidTokenOwner,
        constraint = user_source_bond_account.mint == source_bond_mint.key() @ EvolutionError::InvalidBondMint,
    )]
    pub user_source_bond_account: Box<Account<'info, TokenAccount>>,

    /// CHECK: Source custody from base lp-bonds program OR evolution program.
    /// Validated in handler: owner check, PDA derivation, bond_mint field.
    #[account(mut)]
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
    pub layer_token_authority: Box<Account<'info, LayerTokenAuthority>>,

    #[account(
        mut,
        constraint = user_token_a_account.owner == user.key() @ EvolutionError::InvalidTokenOwner,
        constraint = user_token_a_account.mint == token_mint_a.key() @ EvolutionError::InvalidTokenMint,
    )]
    pub user_token_a_account: Box<Account<'info, TokenAccount>>,

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

    /// CHECK: Validated against level config and owner check in handler
    #[account(
        mut,
        constraint = whirlpool.key() == level_config.whirlpool @ EvolutionError::WhirlpoolLevelMismatch,
    )]
    pub whirlpool: UncheckedAccount<'info>,

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
