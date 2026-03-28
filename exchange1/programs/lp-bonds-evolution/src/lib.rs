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
        require!(oracle_authority != Pubkey::default(), EvolutionError::InvalidEvolutionOracle);

        let config = &mut ctx.accounts.evolution_config;
        config.admin = ctx.accounts.admin.key();
        config.pending_admin = Pubkey::default();
        config.treasury = treasury;
        config.oracle_authority = oracle_authority;
        config.lp_bonds_program_id = lp_bonds_program_id;
        config.is_paused = false;
        config.oracle_enabled = true;
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
        max_total_mint: u64,
        max_mint_per_tx: u64,
    ) -> Result<()> {
        require!(level_id >= 2 && level_id <= MAX_BOND_LEVEL, EvolutionError::InvalidBondLevel);
        require!(fee_bps <= MAX_FEE_BPS, EvolutionError::FeeTooHigh);
        require!(tick_lower < tick_upper, EvolutionError::InvalidTickRange);
        require!(lock_duration > 0, EvolutionError::InvalidLockDuration);
        require!(max_total_mint > 0, EvolutionError::InvalidMintCap);
        require!(max_mint_per_tx > 0, EvolutionError::InvalidMintCap);
        require!(max_mint_per_tx <= max_total_mint, EvolutionError::InvalidMintCap);

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
        config.max_total_mint = max_total_mint;
        config.max_mint_per_tx = max_mint_per_tx;
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

    /// Configure a level via delegated authority (whitelisted with PERM_CONFIGURE_LEVELS).
    pub fn configure_level_delegated(
        ctx: Context<ConfigureLevelDelegated>,
        level_id: u8,
        tick_lower: i32,
        tick_upper: i32,
        required_amount_a: u64,
        required_amount_b: u64,
        fee_bps: u16,
        lock_duration: i64,
        multiplier: u16,
        is_active: bool,
        max_total_mint: u64,
        max_mint_per_tx: u64,
    ) -> Result<()> {
        // Check delegated authority has PERM_CONFIGURE_LEVELS
        require!(
            ctx.accounts.authority_whitelist.permissions & PERM_CONFIGURE_LEVELS != 0,
            EvolutionError::InsufficientPermissions
        );

        require!(level_id >= 2 && level_id <= MAX_BOND_LEVEL, EvolutionError::InvalidBondLevel);
        require!(fee_bps <= MAX_FEE_BPS, EvolutionError::FeeTooHigh);
        require!(tick_lower < tick_upper, EvolutionError::InvalidTickRange);
        require!(lock_duration > 0, EvolutionError::InvalidLockDuration);
        require!(max_total_mint > 0, EvolutionError::InvalidMintCap);
        require!(max_mint_per_tx > 0, EvolutionError::InvalidMintCap);
        require!(max_mint_per_tx <= max_total_mint, EvolutionError::InvalidMintCap);

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
        config.max_total_mint = max_total_mint;
        config.max_mint_per_tx = max_mint_per_tx;
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
            admin: ctx.accounts.caller.key(),
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
        require!(new_treasury != Pubkey::default(), EvolutionError::TreasuryNotSet);

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
        require!(new_oracle != Pubkey::default(), EvolutionError::InvalidEvolutionOracle);

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

    /// Enable or disable the oracle for evolution.
    pub fn set_oracle_enabled(ctx: Context<SetOracleEnabled>, enabled: bool) -> Result<()> {
        ctx.accounts.evolution_config.oracle_enabled = enabled;

        emit!(OracleEnabledChanged {
            enabled,
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
    ///
    /// SECURITY: Nonce accounts are intentionally NOT closable.
    /// This prevents replay attacks via nonce reset.
    /// Users must reuse the same nonce account permanently.
    /// Uses `init` (not `init_if_needed`) so re-initialization of an
    /// existing account will fail at the Anchor level.
    pub fn initialize_evolution_nonce(ctx: Context<InitializeEvolutionNonce>) -> Result<()> {
        let nonce = &mut ctx.accounts.evolution_nonce;
        nonce.user = ctx.accounts.user.key();
        nonce.current_nonce = 0;
        nonce.last_execution_timestamp = 0;
        nonce.bump = ctx.bumps.evolution_nonce;

        emit!(EvolutionNonceInitialized {
            user: ctx.accounts.user.key(),
            timestamp: Clock::get()?.unix_timestamp,
        });

        Ok(())
    }

    /// Emergency token recovery (admin only).
    /// Transfers tokens from a program-controlled account to the admin's token account.
    /// Safety: source account must NOT be a custody position token account.
    pub fn recover_tokens(ctx: Context<RecoverTokens>, amount: u64) -> Result<()> {
        let authority_seeds: &[&[u8]] = &[
            LAYER_TOKEN_AUTHORITY_SEED,
            &[ctx.accounts.layer_token_authority.bump],
        ];
        let signer_seeds = &[authority_seeds];

        token::transfer(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                Transfer {
                    from: ctx.accounts.source_token_account.to_account_info(),
                    to: ctx.accounts.admin_token_account.to_account_info(),
                    authority: ctx.accounts.layer_token_authority.to_account_info(),
                },
                signer_seeds,
            ),
            amount,
        )?;

        emit!(RecoveryEvent {
            token_mint: ctx.accounts.source_token_account.mint,
            amount,
            admin: ctx.accounts.admin.key(),
            timestamp: Clock::get()?.unix_timestamp,
        });

        Ok(())
    }

    /// Collect accumulated fees from a custodied Orca Whirlpool position.
    ///
    /// The user must hold the bond NFT (amount == 1). Fees are collected
    /// via CPI to the Whirlpool program and sent directly to the user's
    /// token accounts. The layer_token_authority PDA signs as position authority.
    pub fn collect_fees(ctx: Context<CollectFees>) -> Result<()> {
        require!(
            !ctx.accounts.evolution_config.is_paused,
            EvolutionError::EvolutionPaused
        );

        // =================================================================
        // TICK ARRAY PDA VALIDATION (defense-in-depth)
        // Matches the validation pattern in validate_whirlpool_and_ticks.
        // =================================================================
        {
            require_keys_eq!(
                ctx.accounts.whirlpool.key(),
                ctx.accounts.position_custody.whirlpool,
                EvolutionError::WhirlpoolLevelMismatch
            );

            let whirlpool_state = whirlpool_cpi::Whirlpool::from_account_info(
                &ctx.accounts.whirlpool.to_account_info(),
            )?;
            let tick_lower = ctx.accounts.position_custody.tick_lower_index;
            let tick_upper = ctx.accounts.position_custody.tick_upper_index;
            let spacing = whirlpool_state.tick_spacing;

            require_keys_eq!(
                *ctx.accounts.tick_array_lower.owner,
                whirlpool_cpi::WHIRLPOOL_PROGRAM_ID,
                EvolutionError::InvalidWhirlpoolProgram
            );
            require_keys_eq!(
                *ctx.accounts.tick_array_upper.owner,
                whirlpool_cpi::WHIRLPOOL_PROGRAM_ID,
                EvolutionError::InvalidWhirlpoolProgram
            );

            let start_lower = whirlpool_cpi::get_start_tick_index(tick_lower, spacing);
            let (expected_lower, _) = whirlpool_cpi::get_tick_array_address(
                &ctx.accounts.whirlpool.key(),
                start_lower,
            );
            require_keys_eq!(
                ctx.accounts.tick_array_lower.key(),
                expected_lower,
                EvolutionError::InvalidTickArrayPda
            );

            let start_upper = whirlpool_cpi::get_start_tick_index(tick_upper, spacing);
            let (expected_upper, _) = whirlpool_cpi::get_tick_array_address(
                &ctx.accounts.whirlpool.key(),
                start_upper,
            );
            require_keys_eq!(
                ctx.accounts.tick_array_upper.key(),
                expected_upper,
                EvolutionError::InvalidTickArrayPda
            );
        }

        // CPI: update_fees_and_rewards to ensure fee accumulators are current
        // before collecting. This matches EVM behavior where fees are always
        // up-to-date at collection time.
        whirlpool_cpi::update_fees_and_rewards(
            &ctx.accounts.whirlpool_program.to_account_info(),
            &ctx.accounts.whirlpool.to_account_info(),
            &ctx.accounts.whirlpool_position.to_account_info(),
            &ctx.accounts.tick_array_lower.to_account_info(),
            &ctx.accounts.tick_array_upper.to_account_info(),
        )?;

        let authority_seeds: &[&[u8]] = &[
            LAYER_TOKEN_AUTHORITY_SEED,
            &[ctx.accounts.layer_token_authority.bump],
        ];
        let signer_seeds = &[authority_seeds];

        // CPI: collect_fees from the Whirlpool position
        whirlpool_cpi::collect_fees(
            &ctx.accounts.whirlpool_program.to_account_info(),
            &ctx.accounts.whirlpool.to_account_info(),
            &ctx.accounts.layer_token_authority.to_account_info(),
            &ctx.accounts.whirlpool_position.to_account_info(),
            &ctx.accounts.custody_position_token_account.to_account_info(),
            &ctx.accounts.user_token_a_account.to_account_info(),
            &ctx.accounts.user_token_b_account.to_account_info(),
            &ctx.accounts.token_vault_a.to_account_info(),
            &ctx.accounts.token_vault_b.to_account_info(),
            &ctx.accounts.token_program.to_account_info(),
            signer_seeds,
        )?;

        emit!(FeesCollected {
            bond_mint: ctx.accounts.bond_mint.key(),
            position_mint: ctx.accounts.position_custody.position_mint,
            fees_a: 0, // Actual amounts determined by Whirlpool CPI
            fees_b: 0,
            collector: ctx.accounts.user.key(),
            timestamp: Clock::get()?.unix_timestamp,
        });

        Ok(())
    }

    /// Close an orphaned PositionCustody account (admin only).
    ///
    /// An orphaned custody is one whose associated bond_mint has supply == 0,
    /// meaning the bond NFT has been burned (e.g., after redemption or evolution).
    /// The underlying Whirlpool position liquidity remains locked by design;
    /// this instruction only reclaims the PDA rent to the admin.
    pub fn close_orphaned_custody(_ctx: Context<CloseOrphanedCustody>) -> Result<()> {
        // Account is closed via `close = admin` constraint on the account struct.
        // The bond_mint supply == 0 constraint ensures the bond has been burned.
        Ok(())
    }

    // =========================================================================
    // EVOLUTION
    // =========================================================================

    /// Evolve a bond to the next level.
    ///
    /// remaining_accounts[0] = tick_array_lower (validated: owner, PDA, coverage)
    /// remaining_accounts[1] = tick_array_upper (validated: owner, PDA, coverage)
    /// remaining_accounts[2] = token_vault_a (validated against whirlpool state)
    /// remaining_accounts[3] = token_vault_b (validated against whirlpool state)
    ///
    /// ## Security guarantees (mirrors locker)
    ///
    /// 1. Ed25519 instruction must be at exactly (current_index - 1)
    /// 2. Oracle pubkey must match configured authority
    /// 3. Nonce must be exactly current_nonce + 1 (strict sequential)
    /// 4. Timestamp must be recent (within MAX_ORACLE_STALENESS_SECONDS)
    /// 5. Tick range from LevelConfig — NOT user-provided
    /// 6. Whirlpool deserialized via from_account_info (owner-checked)
    /// 7. Token mints cross-checked against whirlpool state
    /// 8. Tick arrays validated: owner, PDA derivation, coverage
    /// 9. Post-CPI position validated: whirlpool binding
    pub fn evolve_bond<'info>(
        ctx: Context<'_, '_, 'info, 'info, EvolveBond<'info>>,
        target_level: u8,
        amount_a: u64,
        amount_b: u64,
        liquidity_amount: u128,
        token_max_a: u64,
        token_max_b: u64,
        nonce: u64,
        tick_current: i32,
        oracle_timestamp: i64,
    ) -> Result<()> {
        require!(
            ctx.remaining_accounts.len() == 4,
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

        // =================================================================
        // INPUT VALIDATION — fail fast before any state changes
        // =================================================================
        require!(liquidity_amount > 0, EvolutionError::ZeroLiquidityAmount);
        require!(
            token_max_a > 0 || token_max_b > 0,
            EvolutionError::ZeroTokenAmounts
        );

        // Read enforced tick range from LevelConfig — NOT from user input.
        // This prevents user manipulation of position boundaries.
        let tick_lower_index = ctx.accounts.level_config.tick_lower;
        let tick_upper_index = ctx.accounts.level_config.tick_upper;

        // Enforce minimum amounts from LevelConfig
        require!(
            amount_a >= ctx.accounts.level_config.required_amount_a,
            EvolutionError::InsufficientEvolutionAmount
        );
        require!(
            amount_b >= ctx.accounts.level_config.required_amount_b,
            EvolutionError::InsufficientEvolutionAmount
        );

        // --- SOURCE CUSTODY VALIDATION ---
        // Extracted to separate function for stack optimization.
        let source_level = validate_source_custody(
            &ctx.accounts.source_custody,
            &ctx.accounts.source_bond_mint.key(),
            &ctx.accounts.evolution_config.lp_bonds_program_id,
        )?;

        // --- LEVEL TRANSITION VALIDATION ---
        require!(
            target_level == source_level.checked_add(1).ok_or(EvolutionError::MaxLevelReached)?,
            EvolutionError::InvalidLevelTransition
        );
        require!(target_level <= MAX_BOND_LEVEL, EvolutionError::MaxLevelReached);
        require!(ctx.accounts.level_config.is_active, EvolutionError::LevelNotActive);

        // =================================================================
        // ORACLE VERIFICATION (mandatory — no evolution without attestation)
        // Extracted to separate function for stack optimization.
        // =================================================================

        // SECURITY: Ensure the transaction signer matches the oracle-signed sender.
        // The oracle message is reconstructed using ctx.accounts.user.key() as `sender`.
        // Since `user` is a required Signer account, this guarantees that the
        // entity executing the transaction is the same entity the oracle signed for.
        // This prevents reuse of oracle signatures by third parties.
        require!(
            ctx.accounts.user.is_signer,
            EvolutionError::UnauthorizedSigner
        );

        require!(
            ctx.accounts.evolution_config.oracle_enabled,
            EvolutionError::OracleNotEnabled
        );

        validate_oracle_and_nonce(
            &ctx.accounts.instructions_sysvar,
            &ctx.accounts.evolution_config.oracle_authority,
            ctx.accounts.evolution_nonce.current_nonce,
            nonce,
            now,
            oracle_timestamp,
            ctx.accounts.source_bond_mint.key(),
            target_level,
            ctx.accounts.whirlpool.key(),
            ctx.accounts.token_mint_a.key(),
            ctx.accounts.layer_token_mint.key(),
            amount_a,
            amount_b,
            liquidity_amount,
            tick_lower_index,
            tick_upper_index,
            tick_current,
            ctx.accounts.user.key(),
        )?;

        // Per-user rate limiting: MUST run before nonce mutation to preserve
        // the oracle signature if rate limit fails.
        require!(
            now.saturating_sub(ctx.accounts.evolution_nonce.last_execution_timestamp)
                >= MIN_EVOLUTION_DELAY_SECONDS,
            EvolutionError::EvolutionRateLimited
        );

        // Commit nonce AFTER all validation passes
        {
            let old_nonce = ctx.accounts.evolution_nonce.current_nonce;
            ctx.accounts.evolution_nonce.current_nonce = nonce;

            emit!(EvolutionNonceIncremented {
                user: ctx.accounts.user.key(),
                old_nonce,
                new_nonce: nonce,
                timestamp: now,
            });
        }

        // =================================================================
        // WHIRLPOOL STATE VALIDATION
        // Deserialize whirlpool and cross-validate all externally provided
        // accounts against on-chain state BEFORE any CPI or state changes.
        // Extracted to separate function for stack optimization.
        // =================================================================
        // Belt-and-suspenders: verify whirlpool_program matches the hardcoded
        // constant inside the handler, not just in account constraints.
        require_keys_eq!(
            ctx.accounts.whirlpool_program.key(),
            whirlpool_cpi::WHIRLPOOL_PROGRAM_ID,
            EvolutionError::InvalidWhirlpoolProgram
        );

        validate_whirlpool_and_ticks(
            &ctx.accounts.whirlpool,
            &ctx.accounts.whirlpool.key(),
            &ctx.accounts.level_config.whirlpool,
            &ctx.accounts.token_mint_a.key(),
            &ctx.accounts.layer_token_mint.key(),
            &ctx.accounts.level_config.token_mint_a,
            &ctx.accounts.level_config.token_mint_b,
            &token_vault_a.key(),
            &token_vault_b.key(),
            tick_lower_index,
            tick_upper_index,
            tick_current,
            tick_array_lower,
            tick_array_upper,
        )?;

        // --- BOND OWNERSHIP ---
        require!(
            ctx.accounts.user_source_bond_account.amount == 1,
            EvolutionError::InvalidBondBalance
        );

        // INVARIANT: liquidity_amount, token_max_a, and token_max_b are the
        // exact values signed by the oracle in the Ed25519 instruction. They
        // are passed directly to the Whirlpool CPI without modification. Any
        // transformation would break the oracle binding and could allow CPI
        // manipulation with values different from what the oracle attested.

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

        // Per-transaction mint cap: prevent single-tx supply drain
        require!(
            amount_b <= ctx.accounts.level_config.max_mint_per_tx,
            EvolutionError::MintCapExceeded
        );

        // EBG-09: Enforce per-level mint cap before minting layer tokens
        require!(
            ctx.accounts.level_config.total_minted
                .checked_add(amount_b)
                .ok_or(EvolutionError::ArithmeticOverflow)?
                <= ctx.accounts.level_config.max_total_mint,
            EvolutionError::MintCapExceeded
        );

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

        // EBG-09: Update total_minted after successful mint
        ctx.accounts.level_config.total_minted = ctx.accounts.level_config.total_minted
            .checked_add(amount_b)
            .ok_or(EvolutionError::ArithmeticOverflow)?;

        // STEP 4: Deduct protocol fee (checked arithmetic — no unwrap_or)
        let fee = ctx.accounts.level_config.calculate_fee(amount_a)?;
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

        // STEP 5: Open Whirlpool position (using config ticks)
        // NOTE: No token approve/delegate needed. The layer_token_authority PDA
        // owns the program token accounts and signs the increase_liquidity CPI
        // via invoke_signed. Orca's Whirlpool uses the position_authority
        // (layer_token_authority) as the transfer authority directly.
        // Position mint must be uninitialized (fresh keypair). An already-
        // initialized mint could have non-zero supply, allowing position reuse.
        // Since position_mint is Signer (not Account<Mint>), data_is_empty()
        // is the correct check — equivalent to supply == 0 for a fresh account.
        require!(
            ctx.accounts.position_mint.data_is_empty(),
            EvolutionError::InvalidPositionMint
        );

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

        // Post-CPI: Validate position token account mint and owner.
        // After open_position, the position_token_account is initialized as an
        // ATA holding the position NFT. Verify it's bound to the correct mint
        // and owned by the user (who must transfer it to custody).
        {
            let pta_data = ctx.accounts.position_token_account.data.borrow();
            let pta = anchor_spl::token::TokenAccount::try_deserialize(&mut &pta_data[..])
                .map_err(|_| error!(EvolutionError::InvalidAccountData))?;
            require_keys_eq!(
                pta.mint,
                ctx.accounts.position_mint.key(),
                EvolutionError::InvalidPositionMint
            );
            require_keys_eq!(
                pta.owner,
                ctx.accounts.user.key(),
                EvolutionError::InvalidTokenOwner
            );
        }

        // Post-CPI: Validate the newly created position belongs to our whirlpool.
        // After open_position CPI, the position account is initialized with the
        // whirlpool it was opened against. Verifying this ensures the CPI did not
        // somehow bind to a different pool.
        {
            let pos_data = ctx.accounts.whirlpool_position.data.borrow();
            // Position is an Anchor account with 8-byte discriminator.
            // First field is whirlpool pubkey (32 bytes at offset 8).
            require!(pos_data.len() >= 40, EvolutionError::InvalidAccountData);
            let pos_whirlpool = Pubkey::try_from(&pos_data[8..40])
                .map_err(|_| error!(EvolutionError::InvalidAccountData))?;
            require_keys_eq!(
                pos_whirlpool,
                ctx.accounts.whirlpool.key(),
                EvolutionError::PositionDataMismatch
            );
        }

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

        // STEP 8.5: Return residual tokens after increase_liquidity.
        // The Whirlpool CPI may consume less than deposited due to price movement.
        // Return excess token_a to user; burn excess layer tokens (token_b).
        ctx.accounts.program_token_a_account.reload()?;
        if ctx.accounts.program_token_a_account.amount > 0 {
            token::transfer(
                CpiContext::new_with_signer(
                    ctx.accounts.token_program.to_account_info(),
                    Transfer {
                        from: ctx.accounts.program_token_a_account.to_account_info(),
                        to: ctx.accounts.user_token_a_account.to_account_info(),
                        authority: ctx.accounts.layer_token_authority.to_account_info(),
                    },
                    signer_seeds,
                ),
                ctx.accounts.program_token_a_account.amount,
            )?;
        }

        ctx.accounts.program_token_b_account.reload()?;
        if ctx.accounts.program_token_b_account.amount > 0 {
            token::burn(
                CpiContext::new_with_signer(
                    ctx.accounts.token_program.to_account_info(),
                    Burn {
                        mint: ctx.accounts.layer_token_mint.to_account_info(),
                        from: ctx.accounts.program_token_b_account.to_account_info(),
                        authority: ctx.accounts.layer_token_authority.to_account_info(),
                    },
                    signer_seeds,
                ),
                ctx.accounts.program_token_b_account.amount,
            )?;
        }

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

        // STEP 10: Initialize position custody (using config ticks, NOT user input)
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

        // STEP 11: Create evolution record
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

        // Update rate limit timestamp after successful execution
        ctx.accounts.evolution_nonce.last_execution_timestamp = now;

        Ok(())
    }

    /// Redeem an evolved bond (Level 2-4) to reclaim the underlying position.
    ///
    /// Burns the bond NFT, transfers the position NFT from custody to the user,
    /// and closes the PositionCustody account to return rent.
    /// NOTE: Redemption is NOT pause-gated. Users can always redeem after lock
    /// expiry, matching EVM behavior where admin cannot block withdrawals.
    pub fn redeem_evolved_bond(ctx: Context<RedeemEvolvedBond>) -> Result<()> {
        let custody = &ctx.accounts.position_custody;
        let current_time = Clock::get()?.unix_timestamp;

        // Check lock expiry
        let unlock_time = custody.created_at.saturating_add(custody.lock_duration);
        require!(
            current_time >= unlock_time,
            EvolutionError::BondStillLocked
        );

        // Burn bond NFT
        token::burn(
            CpiContext::new(
                ctx.accounts.token_program.to_account_info(),
                Burn {
                    mint: ctx.accounts.bond_mint.to_account_info(),
                    from: ctx.accounts.user_bond_account.to_account_info(),
                    authority: ctx.accounts.user.to_account_info(),
                },
            ),
            1,
        )?;

        // Transfer position NFT from custody to user via layer_token_authority
        let authority_seeds: &[&[u8]] = &[
            LAYER_TOKEN_AUTHORITY_SEED,
            &[ctx.accounts.layer_token_authority.bump],
        ];
        let signer_seeds = &[authority_seeds];

        token::transfer(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                Transfer {
                    from: ctx.accounts.custody_position_token_account.to_account_info(),
                    to: ctx.accounts.user_position_token_account.to_account_info(),
                    authority: ctx.accounts.layer_token_authority.to_account_info(),
                },
                signer_seeds,
            ),
            1,
        )?;

        emit!(BondRedeemed {
            bond_mint: ctx.accounts.bond_mint.key(),
            position_mint: ctx.accounts.position_custody.position_mint,
            redeemer: ctx.accounts.user.key(),
            level: ctx.accounts.position_custody.level,
            timestamp: current_time,
        });

        Ok(())
    }
}

// =============================================================================
// STACK-OPTIMIZED VALIDATION HELPERS
// =============================================================================

/// Validates source custody: PDA derivation, deserialization via PositionCustodyRef,
/// bond_mint binding. Double-evolution prevention is handled by the EvolutionRecord
/// PDA init constraint (seeded by source_bond_mint).
/// Returns the source level for level transition validation.
#[inline(never)]
fn validate_source_custody(
    source_custody: &AccountInfo,
    source_bond_mint_key: &Pubkey,
    lp_bonds_program_id: &Pubkey,
) -> Result<u8> {
    // Owner must be the base lp-bonds program or this evolution program
    let source_custody_owner = *source_custody.owner;
    require!(
        source_custody_owner == *lp_bonds_program_id || source_custody_owner == crate::ID,
        EvolutionError::InvalidCustodyPda
    );

    // PDA derivation check
    let (expected_custody_pda, _) = Pubkey::find_program_address(
        &[b"position_custody", source_bond_mint_key.as_ref()],
        &source_custody_owner,
    );
    require_keys_eq!(
        source_custody.key(),
        expected_custody_pda,
        EvolutionError::InvalidCustodyPda
    );

    // Deserialize using PositionCustodyRef instead of raw byte slicing
    let custody_data = source_custody.try_borrow_data()?;
    // Skip 8-byte Anchor discriminator
    require!(custody_data.len() >= 8, EvolutionError::InvalidAccountData);
    let custody_ref = PositionCustodyRef::deserialize(&mut &custody_data[8..])
        .map_err(|_| error!(EvolutionError::InvalidAccountData))?;
    drop(custody_data);

    // Validate bond_mint matches
    require_keys_eq!(
        custody_ref.bond_mint,
        *source_bond_mint_key,
        EvolutionError::InvalidBondMint
    );

    // Double-evolution prevention is handled by the EvolutionRecord PDA
    // (seeded by source_bond_mint, using `init` constraint). The `is_evolved`
    // flag was incorrectly blocking L2->L3 and L3->L4 progression because
    // evolved bonds (L2+) have is_evolved == true by design.

    // Validate that the custody has a real whirlpool binding.
    // Trust model:
    //   - Level 1 bonds are created by the base lp-bonds program, which
    //     sets custody.whirlpool to the allowlisted whirlpool during mint.
    //   - Level >1 bonds are created by this evolution program, which
    //     sets custody.whirlpool to the validated target whirlpool.
    // In both cases, the whirlpool field must be non-default. A default
    // (zeroed) whirlpool indicates uninitialized or corrupted custody data.
    require!(
        custody_ref.whirlpool != Pubkey::default(),
        EvolutionError::InvalidAccountData
    );

    // Validate and use source level directly — reject invalid levels
    let source_level = custody_ref.level;
    require!(
        source_level >= MIN_BOND_LEVEL && source_level <= MAX_BOND_LEVEL,
        EvolutionError::InvalidBondLevel
    );

    Ok(source_level)
}

/// Validates whirlpool state, token mints, vaults, and tick correctness.
/// Extracted into a separate function to reduce stack pressure in evolve_bond.
#[inline(never)]
fn validate_whirlpool_and_ticks(
    whirlpool_info: &AccountInfo,
    whirlpool_key: &Pubkey,
    level_config_whirlpool: &Pubkey,
    token_mint_a_key: &Pubkey,
    layer_token_mint_key: &Pubkey,
    level_config_token_mint_a: &Pubkey,
    level_config_token_mint_b: &Pubkey,
    token_vault_a_key: &Pubkey,
    token_vault_b_key: &Pubkey,
    tick_lower_index: i32,
    tick_upper_index: i32,
    tick_current: i32,
    tick_array_lower: &AccountInfo,
    tick_array_upper: &AccountInfo,
) -> Result<()> {
    // Whirlpool::from_account_info internally verifies
    // owner == WHIRLPOOL_PROGRAM_ID. No separate owner check needed.
    let whirlpool_state = whirlpool_cpi::Whirlpool::from_account_info(whirlpool_info)?;

    // Belt-and-suspenders: verify whirlpool matches level config
    // inside the handler, not just in account constraints.
    require_keys_eq!(
        *whirlpool_key,
        *level_config_whirlpool,
        EvolutionError::WhirlpoolLevelMismatch
    );

    // Validate that the oracle-provided tick_current matches the actual
    // on-chain whirlpool state.
    require!(
        tick_current == whirlpool_state.tick_current_index,
        EvolutionError::TickCurrentMismatch
    );

    // Cross-validate token mints against whirlpool on-chain data.
    require_keys_eq!(
        *token_mint_a_key,
        whirlpool_state.token_mint_a,
        EvolutionError::InvalidTokenMint
    );
    require_keys_eq!(
        *layer_token_mint_key,
        whirlpool_state.token_mint_b,
        EvolutionError::InvalidTokenMint
    );

    // Symmetric LevelConfig ↔ Whirlpool binding: verify BOTH
    // level_config.token_mint_a and level_config.token_mint_b match
    // whirlpool state. This catches config-whirlpool desync on either side.
    require_keys_eq!(
        *level_config_token_mint_a,
        whirlpool_state.token_mint_a,
        EvolutionError::InvalidTokenMint
    );
    require_keys_eq!(
        *level_config_token_mint_b,
        whirlpool_state.token_mint_b,
        EvolutionError::InvalidTokenMint
    );

    // Validate token vaults match whirlpool state
    require_keys_eq!(
        *token_vault_a_key,
        whirlpool_state.token_vault_a,
        EvolutionError::InvalidTokenVault
    );
    require_keys_eq!(
        *token_vault_b_key,
        whirlpool_state.token_vault_b,
        EvolutionError::InvalidTokenVault
    );

    // Tick correctness
    require!(tick_lower_index < tick_upper_index, EvolutionError::InvalidTickRange);
    require!(tick_lower_index >= MIN_TICK_INDEX, EvolutionError::TickOutOfBounds);
    require!(tick_upper_index <= MAX_TICK_INDEX, EvolutionError::TickOutOfBounds);

    let spacing = whirlpool_state.tick_spacing as i32;
    require!(
        tick_lower_index % spacing == 0,
        EvolutionError::TickNotAlignedToSpacing
    );
    require!(
        tick_upper_index % spacing == 0,
        EvolutionError::TickNotAlignedToSpacing
    );

    // Tick array owner checks
    require_keys_eq!(
        *tick_array_lower.owner,
        whirlpool_cpi::WHIRLPOOL_PROGRAM_ID,
        EvolutionError::InvalidWhirlpoolProgram
    );
    require_keys_eq!(
        *tick_array_upper.owner,
        whirlpool_cpi::WHIRLPOOL_PROGRAM_ID,
        EvolutionError::InvalidWhirlpoolProgram
    );

    // Tick array PDA derivation
    let start_lower = whirlpool_cpi::get_start_tick_index(
        tick_lower_index, whirlpool_state.tick_spacing,
    );
    let (expected_lower, _) = whirlpool_cpi::get_tick_array_address(whirlpool_key, start_lower);
    require_keys_eq!(
        tick_array_lower.key(),
        expected_lower,
        EvolutionError::InvalidTickArrayPda
    );

    let start_upper = whirlpool_cpi::get_start_tick_index(
        tick_upper_index, whirlpool_state.tick_spacing,
    );
    let (expected_upper, _) = whirlpool_cpi::get_tick_array_address(whirlpool_key, start_upper);
    require_keys_eq!(
        tick_array_upper.key(),
        expected_upper,
        EvolutionError::InvalidTickArrayPda
    );

    // Tick coverage
    let ticks_in_array = whirlpool_cpi::TICK_ARRAY_SIZE * spacing;
    require!(
        tick_lower_index >= start_lower
            && tick_lower_index < start_lower + ticks_in_array,
        EvolutionError::TickOutOfBounds
    );
    require!(
        tick_upper_index >= start_upper
            && tick_upper_index < start_upper + ticks_in_array,
        EvolutionError::TickOutOfBounds
    );

    Ok(())
}

/// Validates oracle nonce + timestamp, constructs and verifies the Ed25519
/// signature. Returns the old nonce for event emission.
#[inline(never)]
fn validate_oracle_and_nonce(
    instructions_sysvar: &AccountInfo,
    oracle_authority: &Pubkey,
    current_nonce: u64,
    nonce: u64,
    now: i64,
    oracle_timestamp: i64,
    source_bond_mint: Pubkey,
    target_level: u8,
    whirlpool: Pubkey,
    token_mint_a: Pubkey,
    token_mint_b: Pubkey,
    amount_a: u64,
    amount_b: u64,
    liquidity: u128,
    tick_lower: i32,
    tick_upper: i32,
    tick_current: i32,
    sender: Pubkey,
) -> Result<()> {
    // Strict nonce: must be exactly current_nonce + 1
    let expected_nonce = current_nonce
        .checked_add(1)
        .ok_or(EvolutionError::ArithmeticOverflow)?;
    require!(nonce == expected_nonce, EvolutionError::InvalidNonceSequence);

    // Timestamp staleness
    let age = now.checked_sub(oracle_timestamp)
        .ok_or(EvolutionError::ArithmeticOverflow)?;
    require!(age >= 0, EvolutionError::OracleTimestampFuture);
    require!(age <= MAX_ORACLE_STALENESS_SECONDS, EvolutionError::OracleTimestampStale);

    let evolution_params = EvolutionCanonicalMessageParams {
        source_bond_mint,
        target_level,
        whirlpool,
        token_mint_a,
        token_mint_b,
        amount_a,
        amount_b,
        liquidity,
        tick_lower,
        tick_upper,
        tick_current,
        nonce,
        timestamp: oracle_timestamp,
        sender,
        contract_address: crate::ID,
    };

    verify_evolution_signature(
        instructions_sysvar,
        oracle_authority,
        &evolution_params,
    )
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
#[instruction(level_id: u8)]
pub struct ConfigureLevelDelegated<'info> {
    #[account(mut)]
    pub caller: Signer<'info>,

    #[account(seeds = [EVOLUTION_CONFIG_SEED], bump = evolution_config.bump)]
    pub evolution_config: Account<'info, EvolutionConfig>,

    #[account(
        seeds = [AUTHORITY_WHITELIST_SEED, caller.key().as_ref()],
        bump = authority_whitelist.bump,
        constraint = authority_whitelist.authority == caller.key() @ EvolutionError::InsufficientPermissions,
    )]
    pub authority_whitelist: Account<'info, AuthorityWhitelist>,

    #[account(
        init_if_needed,
        payer = caller,
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
pub struct SetOracleEnabled<'info> {
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

#[derive(Accounts)]
pub struct RedeemEvolvedBond<'info> {
    #[account(mut)]
    pub user: Signer<'info>,

    #[account(
        seeds = [EVOLUTION_CONFIG_SEED],
        bump = evolution_config.bump,
    )]
    pub evolution_config: Box<Account<'info, EvolutionConfig>>,

    #[account(mut)]
    pub bond_mint: Box<Account<'info, Mint>>,

    #[account(
        constraint = position_mint.key() == position_custody.position_mint @ EvolutionError::InvalidPositionMint,
    )]
    pub position_mint: Box<Account<'info, Mint>>,

    /// Verify the user owns the bond NFT.
    #[account(
        mut,
        constraint = user_bond_account.owner == user.key() @ EvolutionError::InvalidTokenOwner,
        constraint = user_bond_account.mint == bond_mint.key() @ EvolutionError::InvalidBondMint,
        constraint = user_bond_account.amount == 1 @ EvolutionError::InvalidBondBalance,
    )]
    pub user_bond_account: Box<Account<'info, TokenAccount>>,

    #[account(
        init_if_needed,
        payer = user,
        associated_token::mint = position_mint,
        associated_token::authority = user,
    )]
    pub user_position_token_account: Box<Account<'info, TokenAccount>>,

    #[account(
        mut,
        close = user,
        seeds = [POSITION_CUSTODY_SEED, bond_mint.key().as_ref()],
        bump = position_custody.bump,
        constraint = position_custody.bond_mint == bond_mint.key() @ EvolutionError::InvalidCustodyBondMint,
    )]
    pub position_custody: Box<Account<'info, PositionCustody>>,

    #[account(
        mut,
        constraint = custody_position_token_account.owner == layer_token_authority.key() @ EvolutionError::InvalidTokenOwner,
        constraint = custody_position_token_account.mint == position_mint.key() @ EvolutionError::InvalidPositionMint,
    )]
    pub custody_position_token_account: Box<Account<'info, TokenAccount>>,

    #[account(seeds = [LAYER_TOKEN_AUTHORITY_SEED], bump = layer_token_authority.bump)]
    pub layer_token_authority: Account<'info, LayerTokenAuthority>,

    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
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
    nonce: u64,
    tick_current: i32,
    oracle_timestamp: i64,
)]
pub struct EvolveBond<'info> {
    #[account(mut)]
    pub user: Signer<'info>,

    #[account(mut, seeds = [EVOLUTION_CONFIG_SEED], bump = evolution_config.bump)]
    pub evolution_config: Box<Account<'info, EvolutionConfig>>,

    #[account(mut, seeds = [LEVEL_CONFIG_SEED, &[target_level]], bump = level_config.bump)]
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
    /// Read-only: source custody is not modified during evolution.
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
        constraint = treasury_token_account.mint == token_mint_a.key() @ EvolutionError::InvalidTokenMint,
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

#[derive(Accounts)]
pub struct RecoverTokens<'info> {
    #[account(
        mut,
        constraint = admin.key() == evolution_config.admin @ EvolutionError::InvalidAdminAuthority,
    )]
    pub admin: Signer<'info>,

    #[account(seeds = [EVOLUTION_CONFIG_SEED], bump = evolution_config.bump)]
    pub evolution_config: Account<'info, EvolutionConfig>,

    #[account(seeds = [LAYER_TOKEN_AUTHORITY_SEED], bump = layer_token_authority.bump)]
    pub layer_token_authority: Account<'info, LayerTokenAuthority>,

    /// Source token account controlled by the program (owned by layer_token_authority PDA).
    /// Bound to position_custody via mint == position_custody.position_mint.
    #[account(
        mut,
        constraint = source_token_account.owner == layer_token_authority.key() @ EvolutionError::InvalidTokenOwner,
        constraint = source_token_account.mint == position_custody.position_mint @ EvolutionError::InvalidPositionMint,
    )]
    pub source_token_account: Account<'info, TokenAccount>,

    /// Bond mint associated with the custody being recovered.
    /// Must have supply == 0, proving the bond has been burned and the position
    /// is no longer active. This prevents draining custody position token accounts
    /// that hold active position NFTs.
    #[account(constraint = bond_mint.supply == 0 @ EvolutionError::RecoveryCustodyProtected)]
    pub bond_mint: Account<'info, Mint>,

    /// PositionCustody PDA that binds bond_mint to position_mint.
    /// Ensures the supply == 0 check on bond_mint applies to the actual
    /// position whose tokens are being recovered — prevents passing an
    /// unrelated burned mint to bypass the check.
    #[account(
        seeds = [POSITION_CUSTODY_SEED, bond_mint.key().as_ref()],
        bump = position_custody.bump,
        constraint = position_custody.bond_mint == bond_mint.key() @ EvolutionError::InvalidCustodyBondMint,
    )]
    pub position_custody: Account<'info, PositionCustody>,

    /// Admin's token account to receive recovered tokens.
    #[account(mut)]
    pub admin_token_account: Account<'info, TokenAccount>,

    pub token_program: Program<'info, Token>,
}

#[derive(Accounts)]
pub struct CollectFees<'info> {
    #[account(mut)]
    pub user: Signer<'info>,

    #[account(
        seeds = [EVOLUTION_CONFIG_SEED],
        bump = evolution_config.bump,
    )]
    pub evolution_config: Box<Account<'info, EvolutionConfig>>,

    /// User must hold exactly 1 bond NFT to collect fees.
    #[account(
        constraint = user_bond_account.owner == user.key() @ EvolutionError::InvalidTokenOwner,
        constraint = user_bond_account.mint == bond_mint.key() @ EvolutionError::InvalidBondMint,
        constraint = user_bond_account.amount == 1 @ EvolutionError::InvalidBondBalance,
    )]
    pub user_bond_account: Box<Account<'info, TokenAccount>>,

    pub bond_mint: Box<Account<'info, Mint>>,

    #[account(constraint = position_mint.key() == position_custody.position_mint @ EvolutionError::InvalidPositionMint)]
    pub position_mint: Box<Account<'info, Mint>>,

    #[account(
        seeds = [POSITION_CUSTODY_SEED, bond_mint.key().as_ref()],
        bump = position_custody.bump,
        constraint = position_custody.bond_mint == bond_mint.key() @ EvolutionError::InvalidCustodyBondMint,
    )]
    pub position_custody: Box<Account<'info, PositionCustody>>,

    #[account(
        constraint = custody_position_token_account.owner == layer_token_authority.key() @ EvolutionError::InvalidTokenOwner,
        constraint = custody_position_token_account.mint == position_mint.key() @ EvolutionError::InvalidPositionMint,
    )]
    pub custody_position_token_account: Box<Account<'info, TokenAccount>>,

    #[account(seeds = [LAYER_TOKEN_AUTHORITY_SEED], bump = layer_token_authority.bump)]
    pub layer_token_authority: Account<'info, LayerTokenAuthority>,

    /// CHECK: Whirlpool position account (Orca PDA).
    #[account(mut)]
    pub whirlpool_position: UncheckedAccount<'info>,

    /// CHECK: Validated against custody whirlpool
    #[account(
        constraint = whirlpool.key() == position_custody.whirlpool @ EvolutionError::WhirlpoolLevelMismatch,
    )]
    pub whirlpool: UncheckedAccount<'info>,

    /// User's token A account to receive fees.
    #[account(
        mut,
        constraint = user_token_a_account.owner == user.key() @ EvolutionError::InvalidTokenOwner,
    )]
    pub user_token_a_account: Box<Account<'info, TokenAccount>>,

    /// User's token B account to receive fees.
    #[account(
        mut,
        constraint = user_token_b_account.owner == user.key() @ EvolutionError::InvalidTokenOwner,
    )]
    pub user_token_b_account: Box<Account<'info, TokenAccount>>,

    /// Whirlpool token A vault.
    #[account(mut)]
    pub token_vault_a: Box<Account<'info, TokenAccount>>,

    /// Whirlpool token B vault.
    #[account(mut)]
    pub token_vault_b: Box<Account<'info, TokenAccount>>,

    /// CHECK: Tick array containing the position's lower tick.
    /// Validated by the Whirlpool program during update_fees_and_rewards CPI.
    pub tick_array_lower: UncheckedAccount<'info>,

    /// CHECK: Tick array containing the position's upper tick.
    /// Validated by the Whirlpool program during update_fees_and_rewards CPI.
    pub tick_array_upper: UncheckedAccount<'info>,

    /// CHECK: Orca Whirlpool program
    #[account(address = whirlpool_cpi::WHIRLPOOL_PROGRAM_ID @ EvolutionError::InvalidWhirlpoolProgram)]
    pub whirlpool_program: UncheckedAccount<'info>,

    pub token_program: Program<'info, Token>,
}

#[derive(Accounts)]
pub struct CloseOrphanedCustody<'info> {
    #[account(
        mut,
        constraint = admin.key() == evolution_config.admin @ EvolutionError::InvalidAdminAuthority,
    )]
    pub admin: Signer<'info>,

    #[account(seeds = [EVOLUTION_CONFIG_SEED], bump = evolution_config.bump)]
    pub evolution_config: Account<'info, EvolutionConfig>,

    /// The bond mint associated with the custody. Must have supply == 0
    /// (the bond NFT has been burned).
    #[account(constraint = bond_mint.supply == 0 @ EvolutionError::InvalidBondBalance)]
    pub bond_mint: Account<'info, Mint>,

    /// The orphaned PositionCustody PDA to close. Rent is returned to admin.
    #[account(
        mut,
        close = admin,
        seeds = [POSITION_CUSTODY_SEED, bond_mint.key().as_ref()],
        bump = position_custody.bump,
        constraint = position_custody.bond_mint == bond_mint.key() @ EvolutionError::InvalidCustodyBondMint,
    )]
    pub position_custody: Account<'info, PositionCustody>,
}
