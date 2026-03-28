use anchor_lang::prelude::*;
use anchor_lang::solana_program::{program::invoke, system_instruction};
use anchor_spl::{
    associated_token::AssociatedToken,
    token::{self, Burn, Mint, MintTo, SyncNative, Token, TokenAccount, Transfer},
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

declare_id!("Hjba1MCsx8WUtuVSyYY8QFvTzEjxsTPAUrkwJPTgQJf8");

/// ============================================================================
/// LP BONDS - LEVEL 1 LOCKER PROGRAM
/// ============================================================================
///
/// Handles Level 1 LP Bond creation and redemption:
/// - Add liquidity to Orca Whirlpool and mint bond NFT
/// - Redeem bond NFT to reclaim position
/// - Oracle-based collateral verification
/// - Configurable admin, pause, whirlpool, tick range, and token settings
///
/// Evolution to Level 2-4 is handled by the separate lp-bonds-evolution program.
///
/// PDA STRUCTURE:
/// - Protocol Config: [b"config"]
/// - Position Custody: [b"position_custody", bond_mint]
/// - Bond Authority: [b"bond_authority"]
/// - Oracle Config: [b"oracle_config"]
/// - Nonce Account: [b"nonce", user]
/// ============================================================================

#[program]
pub mod lp_bonds {
    use super::*;

    /// Initialize protocol configuration with configurable parameters.
    ///
    /// The admin sets the tick range that ALL positions in this protocol will use.
    /// This prevents user-controlled tick manipulation and ensures uniform positions.
    pub fn initialize(
        ctx: Context<Initialize>,
        whirlpool: Pubkey,
        token_mint_a: Pubkey,
        token_mint_b: Pubkey,
        tick_lower_index: i32,
        tick_upper_index: i32,
        lock_duration: i64,
    ) -> Result<()> {
        require!(tick_lower_index < tick_upper_index, LpBondsError::InvalidTickRange);
        require!(tick_lower_index >= MIN_TICK_INDEX, LpBondsError::TickOutOfBounds);
        require!(tick_upper_index <= MAX_TICK_INDEX, LpBondsError::TickOutOfBounds);
        require!(lock_duration > 0, LpBondsError::InvalidLockDuration);

        let config = &mut ctx.accounts.config;
        config.admin = ctx.accounts.admin.key();
        config.pending_admin = Pubkey::default();
        config.allowlisted_whirlpool = whirlpool;
        config.token_mint_a = token_mint_a;
        config.token_mint_b = token_mint_b;
        config.tick_lower_index = tick_lower_index;
        config.tick_upper_index = tick_upper_index;
        config.lock_duration = lock_duration;
        config.bond_counter = 0;
        config.is_paused = false;
        config.bump = ctx.bumps.config;

        emit!(ProtocolInitialized {
            admin: config.admin,
            allowlisted_whirlpool: config.allowlisted_whirlpool,
            token_mint_a: config.token_mint_a,
            token_mint_b: config.token_mint_b,
            lock_duration: config.lock_duration,
        });

        Ok(())
    }

    // =========================================================================
    // ADMIN INSTRUCTIONS
    // =========================================================================

    /// Update protocol configuration (admin only).
    pub fn update_config(
        ctx: Context<AdminOnly>,
        whirlpool: Pubkey,
        token_mint_a: Pubkey,
        token_mint_b: Pubkey,
        tick_lower_index: i32,
        tick_upper_index: i32,
        lock_duration: i64,
    ) -> Result<()> {
        require!(tick_lower_index < tick_upper_index, LpBondsError::InvalidTickRange);
        require!(tick_lower_index >= MIN_TICK_INDEX, LpBondsError::TickOutOfBounds);
        require!(tick_upper_index <= MAX_TICK_INDEX, LpBondsError::TickOutOfBounds);
        require!(lock_duration > 0, LpBondsError::InvalidLockDuration);

        let config = &mut ctx.accounts.config;
        config.allowlisted_whirlpool = whirlpool;
        config.token_mint_a = token_mint_a;
        config.token_mint_b = token_mint_b;
        config.tick_lower_index = tick_lower_index;
        config.tick_upper_index = tick_upper_index;
        config.lock_duration = lock_duration;

        emit!(ConfigUpdated {
            admin: ctx.accounts.admin.key(),
            allowlisted_whirlpool: whirlpool,
            token_mint_a,
            token_mint_b,
            lock_duration,
            timestamp: Clock::get()?.unix_timestamp,
        });

        Ok(())
    }

    /// Pause protocol operations.
    pub fn pause(ctx: Context<AdminOnly>) -> Result<()> {
        ctx.accounts.config.is_paused = true;
        emit!(ProtocolPausedEvent {
            admin: ctx.accounts.admin.key(),
            timestamp: Clock::get()?.unix_timestamp,
        });
        Ok(())
    }

    /// Unpause protocol operations.
    pub fn unpause(ctx: Context<AdminOnly>) -> Result<()> {
        ctx.accounts.config.is_paused = false;
        emit!(ProtocolUnpausedEvent {
            admin: ctx.accounts.admin.key(),
            timestamp: Clock::get()?.unix_timestamp,
        });
        Ok(())
    }

    /// Propose admin transfer (two-step pattern).
    pub fn propose_admin(ctx: Context<AdminOnly>, new_admin: Pubkey) -> Result<()> {
        let config = &mut ctx.accounts.config;
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
        let config = &mut ctx.accounts.config;
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

    // =========================================================================
    // BOND INSTRUCTIONS
    // =========================================================================

    /// Add liquidity to Orca Whirlpool and mint Level 1 bond NFT.
    ///
    /// ## Oracle verification (mandatory)
    ///
    /// The transaction MUST include an Ed25519SigVerify instruction **immediately
    /// before** this instruction (at `current_index - 1`). The Ed25519 instruction
    /// must contain a valid oracle signature over the canonical mint message.
    ///
    /// **No signature parameter is accepted.** The signature exists only inside
    /// the Ed25519 instruction. The Solana runtime verifies it cryptographically;
    /// this program verifies the signed message matches the expected parameters.
    ///
    /// ## Tick range
    ///
    /// Tick indices are NOT user-provided. They come from `ProtocolConfig`, which
    /// is set by the admin. This ensures all positions for the same pair use the
    /// same tick range, preventing user manipulation of position boundaries.
    ///
    /// ## Security guarantees
    ///
    /// 1. Oracle must be enabled
    /// 2. Ed25519 instruction must be at exactly (current_index - 1)
    /// 3. Oracle pubkey must match configured authority
    /// 4. Nonce must be exactly current_nonce + 1 (strict sequential)
    /// 5. Timestamp must be recent (within MAX_ORACLE_STALENESS_SECONDS)
    /// 6. Signed message must include whirlpool, both token mints, amounts,
    ///    liquidity, ticks, nonce, timestamp, sender, and program ID
    pub fn add_liquidity_and_mint_bond(
        ctx: Context<AddLiquidityAndMintBond>,
        liquidity_amount: u128,
        token_max_a: u64,
        token_max_b: u64,
        tick_current: i32,
        oracle_nonce: u64,
        oracle_timestamp: i64,
    ) -> Result<()> {
        require!(!ctx.accounts.config.is_paused, LpBondsError::ProtocolPaused);

        // =================================================================
        // INPUT VALIDATION — fail fast before any state changes
        // =================================================================

        // Liquidity and token amount sanity: reject no-op or invalid operations.
        // A zero-liquidity mint would create a bond backed by nothing.
        require!(liquidity_amount > 0, LpBondsError::ZeroLiquidityAmount);
        // At least one token must be deposited — otherwise the position
        // is economically empty regardless of the liquidity parameter.
        require!(
            token_max_a > 0 || token_max_b > 0,
            LpBondsError::ZeroTokenAmounts
        );

        // Read enforced tick range from config — NOT from user input
        let tick_lower_index = ctx.accounts.config.tick_lower_index;
        let tick_upper_index = ctx.accounts.config.tick_upper_index;

        let now = Clock::get()?.unix_timestamp;

        // =================================================================
        // ORACLE VERIFICATION (mandatory — no minting without attestation)
        // =================================================================
        {
            let oracle_config = &ctx.accounts.oracle_config;
            require!(oracle_config.enabled, LpBondsError::OracleNotEnabled);

            // Strict nonce: must be exactly current_nonce + 1.
            // This prevents both replay (same nonce) and skipping (gaps).
            let expected_nonce = ctx.accounts.nonce_account.current_nonce
                .checked_add(1)
                .ok_or(LpBondsError::ArithmeticOverflow)?;
            require!(oracle_nonce == expected_nonce, LpBondsError::InvalidNonceSequence);

            // Timestamp staleness: reject stale or future timestamps
            let age = now.checked_sub(oracle_timestamp)
                .ok_or(LpBondsError::ArithmeticOverflow)?;
            require!(age >= 0, LpBondsError::OracleTimestampFuture);
            require!(age <= MAX_ORACLE_STALENESS_SECONDS, LpBondsError::OracleTimestampStale);

            // Reconstruct the exact message the oracle must have signed.
            // Includes token mints for cross-pair binding.
            let expected_message = reconstruct_oracle_message(&OracleMessageParams {
                domain: ORACLE_DOMAIN_MINT,
                whirlpool: ctx.accounts.whirlpool.key(),
                token_mint_a: ctx.accounts.token_mint_a.key(),
                token_mint_b: ctx.accounts.token_mint_b.key(),
                amount_a: token_max_a,
                amount_b: token_max_b,
                liquidity: liquidity_amount,
                tick_lower: tick_lower_index,
                tick_upper: tick_upper_index,
                tick_current,
                nonce: oracle_nonce,
                timestamp: oracle_timestamp,
                sender: ctx.accounts.user.key(),
                contract_address: crate::ID,
            });

            // SECURITY: Ensure the transaction signer matches the oracle-signed sender.
            // The oracle message is reconstructed using ctx.accounts.user.key() as `sender`.
            // Since `user` is a required Signer account, this guarantees that the
            // entity executing the transaction is the same entity the oracle signed for.
            // This prevents reuse of oracle signatures by third parties.
            require!(
                ctx.accounts.user.is_signer,
                LpBondsError::UnauthorizedSigner
            );

            // Verify Ed25519 instruction at (current_index - 1).
            // No signature parameter — extracted from Ed25519 instruction data.
            verify_oracle_attestation(
                &ctx.accounts.instructions_sysvar,
                &oracle_config.oracle_authority,
                &expected_message,
            )?;

            // Commit nonce AFTER successful verification
            let old_nonce = ctx.accounts.nonce_account.current_nonce;
            ctx.accounts.nonce_account.current_nonce = oracle_nonce;

            emit!(NonceIncremented {
                user: ctx.accounts.user.key(),
                old_nonce,
                new_nonce: oracle_nonce,
                timestamp: now,
            });
        }

        // =================================================================
        // WHIRLPOOL STATE VALIDATION
        // Deserialize whirlpool and cross-validate all externally provided
        // accounts against on-chain state BEFORE any CPI or state changes.
        // =================================================================
        {
            // Whirlpool::from_account_info internally verifies
            // owner == WHIRLPOOL_PROGRAM_ID (hardcoded constant). No separate
            // owner check needed — the constant is authoritative.
            let whirlpool_state = whirlpool_cpi::Whirlpool::from_account_info(
                &ctx.accounts.whirlpool
            )?;

            // Belt-and-suspenders: verify whirlpool matches config allowlist
            // inside the handler, not just in account constraints. This defends
            // against future refactors that might weaken the constraint.
            require_keys_eq!(
                ctx.accounts.whirlpool.key(),
                ctx.accounts.config.allowlisted_whirlpool,
                LpBondsError::WhirlpoolNotAllowlisted
            );

            // Validate that the oracle-provided tick_current matches the actual
            // on-chain whirlpool state. This ensures the oracle's price
            // attestation is consistent with the pool's current tick at
            // execution time.
            require!(
                tick_current == whirlpool_state.tick_current_index,
                LpBondsError::TickCurrentMismatch
            );

            // Cross-validate token mints against whirlpool on-chain data.
            // Config constraints already check mints match config, but we also
            // verify against actual whirlpool state to prevent config-whirlpool
            // desync attacks.
            require_keys_eq!(
                ctx.accounts.token_mint_a.key(),
                whirlpool_state.token_mint_a,
                LpBondsError::InvalidTokenMintA
            );
            require_keys_eq!(
                ctx.accounts.token_mint_b.key(),
                whirlpool_state.token_mint_b,
                LpBondsError::InvalidTokenMintB
            );

            // Validate token vaults match whirlpool state — prevents CPI to
            // drain tokens from non-pool vaults.
            require_keys_eq!(
                ctx.accounts.token_vault_a.key(),
                whirlpool_state.token_vault_a,
                LpBondsError::InvalidTokenVault
            );
            require_keys_eq!(
                ctx.accounts.token_vault_b.key(),
                whirlpool_state.token_vault_b,
                LpBondsError::InvalidTokenVault
            );

            require_keys_eq!(
                ctx.accounts.user_token_a_account.mint,
                whirlpool_state.token_mint_a,
                LpBondsError::InvalidTokenMintA
            );
            require_keys_eq!(
                ctx.accounts.user_token_b_account.mint,
                whirlpool_state.token_mint_b,
                LpBondsError::InvalidTokenMintB
            );

            require_keys_eq!(
                ctx.accounts.token_vault_a.mint,
                whirlpool_state.token_mint_a,
                LpBondsError::InvalidTokenMintA
            );
            require_keys_eq!(
                ctx.accounts.token_vault_b.mint,
                whirlpool_state.token_mint_b,
                LpBondsError::InvalidTokenMintB
            );

            // =============================================================
            // TICK CORRECTNESS VALIDATION
            // Tick spacing alignment is critical: unaligned ticks cause
            // silent failures or opaque reverts in Whirlpool CPI. Validating
            // here provides a clear error message before the CPI is attempted.
            // =============================================================
            require!(tick_lower_index < tick_upper_index, LpBondsError::InvalidTickRange);
            require!(tick_lower_index >= MIN_TICK_INDEX, LpBondsError::TickOutOfBounds);
            require!(tick_upper_index <= MAX_TICK_INDEX, LpBondsError::TickOutOfBounds);

            // Tick spacing alignment: Orca Whirlpool requires tick indices to
            // be exact multiples of the pool's tick_spacing. Unaligned ticks
            // would cause the open_position CPI to fail with an opaque error.
            let spacing = whirlpool_state.tick_spacing as i32;
            require!(
                tick_lower_index % spacing == 0,
                LpBondsError::TickNotAlignedToSpacing
            );
            require!(
                tick_upper_index % spacing == 0,
                LpBondsError::TickNotAlignedToSpacing
            );

            // =============================================================
            // TICK ARRAY PDA VALIDATION (CRITICAL)
            // Tick arrays are externally provided UncheckedAccounts. Without
            // PDA derivation verification, an attacker could supply arbitrary
            // accounts owned by the Whirlpool program, causing the CPI to
            // operate on incorrect tick data. We derive the expected PDAs
            // from the whirlpool address and tick start indices, then compare.
            // =============================================================

            // Tick arrays must be owned by the Whirlpool program.
            // Use the hardcoded WHIRLPOOL_PROGRAM_ID constant for consistency
            // rather than the account reference (already address-constrained).
            require_keys_eq!(
                *ctx.accounts.tick_array_lower.owner,
                whirlpool_cpi::WHIRLPOOL_PROGRAM_ID,
                LpBondsError::InvalidWhirlpoolProgram
            );
            require_keys_eq!(
                *ctx.accounts.tick_array_upper.owner,
                whirlpool_cpi::WHIRLPOOL_PROGRAM_ID,
                LpBondsError::InvalidWhirlpoolProgram
            );

            // Derive expected tick array PDAs and verify they match provided accounts
            let start_lower = whirlpool_cpi::get_start_tick_index(
                tick_lower_index, whirlpool_state.tick_spacing,
            );
            let (expected_lower, _) = whirlpool_cpi::get_tick_array_address(
                &ctx.accounts.whirlpool.key(),
                start_lower,
            );
            require_keys_eq!(
                ctx.accounts.tick_array_lower.key(),
                expected_lower,
                LpBondsError::InvalidTickArrayPda
            );

            let start_upper = whirlpool_cpi::get_start_tick_index(
                tick_upper_index, whirlpool_state.tick_spacing,
            );
            let (expected_upper, _) = whirlpool_cpi::get_tick_array_address(
                &ctx.accounts.whirlpool.key(),
                start_upper,
            );
            require_keys_eq!(
                ctx.accounts.tick_array_upper.key(),
                expected_upper,
                LpBondsError::InvalidTickArrayPda
            );

            // Verify tick indices actually fall within their derived tick
            // arrays. This is a sanity invariant of get_start_tick_index —
            // making it explicit guards against edge-case arithmetic errors.
            let ticks_in_array = whirlpool_cpi::TICK_ARRAY_SIZE * spacing;
            require!(
                tick_lower_index >= start_lower
                    && tick_lower_index < start_lower + ticks_in_array,
                LpBondsError::TickOutOfBounds
            );
            require!(
                tick_upper_index >= start_upper
                    && tick_upper_index < start_upper + ticks_in_array,
                LpBondsError::TickOutOfBounds
            );
        }

        // INVARIANT: liquidity_amount, token_max_a, and token_max_b are the
        // exact values signed by the oracle in the Ed25519 instruction. They
        // are passed directly to the Whirlpool CPI without modification. Any
        // transformation would break the oracle binding and could allow CPI
        // manipulation with values different from what the oracle attested.

        // Wrap native SOL if needed
        maybe_wrap_native_if_needed(
            &ctx.accounts.user,
            &ctx.accounts.token_program,
            ctx.accounts.token_mint_a.key(),
            ctx.accounts.user_token_a_account.as_mut(),
            token_max_a,
        )?;
        maybe_wrap_native_if_needed(
            &ctx.accounts.user,
            &ctx.accounts.token_program,
            ctx.accounts.token_mint_b.key(),
            ctx.accounts.user_token_b_account.as_mut(),
            token_max_b,
        )?;

        // STEP 1: Open Whirlpool position via CPI (using config ticks)
        let (position_pda, position_bump) = whirlpool_cpi::get_position_address(
            &ctx.accounts.position_mint.key(),
        );

        require_keys_eq!(
            ctx.accounts.whirlpool_position.key(),
            position_pda,
            LpBondsError::InvalidPositionPda
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

        // Validate position_token_account mint
        {
            let pta_data = ctx.accounts.position_token_account.data.borrow();
            let pta = anchor_spl::token::TokenAccount::try_deserialize(&mut &pta_data[..])?;
            require_keys_eq!(
                pta.mint,
                ctx.accounts.position_mint.key(),
                LpBondsError::InvalidTokenMint
            );
        }

        // Validate the newly created position belongs to our whirlpool.
        // After open_position CPI, the position account is initialized with
        // the whirlpool it was opened against. Verifying this ensures the
        // CPI did not somehow bind to a different pool.
        {
            let pos_data = ctx.accounts.whirlpool_position.data.borrow();
            // Position is an Anchor account with 8-byte discriminator.
            // First field is whirlpool pubkey (32 bytes at offset 8).
            require!(pos_data.len() >= 40, LpBondsError::InvalidAccountData);
            let pos_whirlpool = Pubkey::try_from(&pos_data[8..40])
                .map_err(|_| error!(LpBondsError::InvalidAccountData))?;
            require_keys_eq!(
                pos_whirlpool,
                ctx.accounts.whirlpool.key(),
                LpBondsError::PositionDataMismatch
            );
        }

        // STEP 2: Create custody position token account
        anchor_spl::associated_token::create(CpiContext::new(
            ctx.accounts.associated_token_program.to_account_info(),
            anchor_spl::associated_token::Create {
                payer: ctx.accounts.user.to_account_info(),
                associated_token: ctx.accounts.custody_position_token_account.to_account_info(),
                authority: ctx.accounts.position_custody.to_account_info(),
                mint: ctx.accounts.position_mint.to_account_info(),
                system_program: ctx.accounts.system_program.to_account_info(),
                token_program: ctx.accounts.token_program.to_account_info(),
            },
        ))?;

        // STEP 3: Add liquidity via CPI
        whirlpool_cpi::increase_liquidity(
            &ctx.accounts.whirlpool_program.to_account_info(),
            &ctx.accounts.whirlpool.to_account_info(),
            &ctx.accounts.token_program.to_account_info(),
            &ctx.accounts.user.to_account_info(),
            &ctx.accounts.whirlpool_position.to_account_info(),
            &ctx.accounts.position_token_account.to_account_info(),
            &ctx.accounts.user_token_a_account.to_account_info(),
            &ctx.accounts.user_token_b_account.to_account_info(),
            &ctx.accounts.token_vault_a.to_account_info(),
            &ctx.accounts.token_vault_b.to_account_info(),
            &ctx.accounts.tick_array_lower.to_account_info(),
            &ctx.accounts.tick_array_upper.to_account_info(),
            liquidity_amount,
            token_max_a,
            token_max_b,
            &[],
        )?;

        // STEP 4: Transfer position NFT to custody
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

        // STEP 5: Mint bond NFT
        let bond_authority_seeds: &[&[u8]] = &[
            BOND_AUTHORITY_SEED,
            &[ctx.bumps.bond_authority],
        ];
        let signer_seeds = &[bond_authority_seeds];

        token::mint_to(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                MintTo {
                    mint: ctx.accounts.bond_mint.to_account_info(),
                    to: ctx.accounts.user_bond_account.to_account_info(),
                    authority: ctx.accounts.bond_authority.to_account_info(),
                },
                signer_seeds,
            ),
            1,
        )?;

        // STEP 6: Update protocol state
        let config = &mut ctx.accounts.config;
        config.bond_counter = config.bond_counter
            .checked_add(1)
            .ok_or(LpBondsError::ArithmeticOverflow)?;

        let lock_duration = config.lock_duration;

        // STEP 7: Initialize position custody
        let custody = &mut ctx.accounts.position_custody;
        custody.bond_mint = ctx.accounts.bond_mint.key();
        custody.position_mint = ctx.accounts.position_mint.key();
        custody.whirlpool = ctx.accounts.whirlpool.key();
        custody.tick_lower_index = tick_lower_index;
        custody.tick_upper_index = tick_upper_index;
        custody.liquidity = liquidity_amount;
        custody.depositor = ctx.accounts.user.key();
        custody.created_at = now;
        custody.level = 1;
        custody.lock_duration = lock_duration;
        custody.is_evolved = false;
        custody.evolved_from = Pubkey::default();
        custody.bump = ctx.bumps.position_custody;
        custody.position_bump = position_bump;

        emit!(BondMinted {
            bond_mint: ctx.accounts.bond_mint.key(),
            position_mint: ctx.accounts.position_mint.key(),
            whirlpool: ctx.accounts.whirlpool.key(),
            depositor: ctx.accounts.user.key(),
            tick_lower_index,
            tick_upper_index,
            liquidity: liquidity_amount,
            token_max_a,
            token_max_b,
            level: 1,
            lock_duration,
            timestamp: now,
        });

        emit!(OracleVerifiedForMint {
            bond_mint: ctx.accounts.bond_mint.key(),
            whirlpool: ctx.accounts.whirlpool.key(),
            sender: ctx.accounts.user.key(),
            token_max_a,
            token_max_b,
            liquidity: liquidity_amount,
            tick_lower: tick_lower_index,
            tick_upper: tick_upper_index,
            tick_current,
            nonce: oracle_nonce,
            oracle_timestamp,
            oracle_authority: ctx.accounts.oracle_config.oracle_authority,
            timestamp: now,
        });

        Ok(())
    }

    /// Redeem bond NFT to reclaim the underlying position.
    /// NOTE: Redemption is NOT pause-gated. Users can always redeem after lock
    /// expiry, matching EVM behavior where admin cannot block withdrawals.
    pub fn redeem_bond(ctx: Context<RedeemBond>) -> Result<()> {
        let custody = &ctx.accounts.position_custody;
        let current_time = Clock::get()?.unix_timestamp;
        require!(
            custody.is_lock_expired(current_time),
            LpBondsError::BondStillLocked
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

        // Transfer position NFT to user
        let bond_mint_key = ctx.accounts.bond_mint.key();
        let custody_seeds: &[&[u8]] = &[
            POSITION_CUSTODY_SEED,
            bond_mint_key.as_ref(),
            &[ctx.accounts.position_custody.bump],
        ];
        let signer_seeds = &[custody_seeds];

        token::transfer(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                Transfer {
                    from: ctx.accounts.custody_position_token_account.to_account_info(),
                    to: ctx.accounts.user_position_token_account.to_account_info(),
                    authority: ctx.accounts.position_custody.to_account_info(),
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

    // =========================================================================
    // ORACLE INSTRUCTIONS
    // =========================================================================

    /// Initialize oracle configuration.
    pub fn initialize_oracle(
        ctx: Context<InitializeOracle>,
        oracle_authority: Pubkey,
    ) -> Result<()> {
        require!(oracle_authority != Pubkey::default(), LpBondsError::InvalidOracleAuthority);

        let oracle_config = &mut ctx.accounts.oracle_config;
        oracle_config.oracle_authority = oracle_authority;
        oracle_config.admin = ctx.accounts.admin.key();
        oracle_config.enabled = true;
        oracle_config.bump = ctx.bumps.oracle_config;

        emit!(OracleInitialized {
            oracle_authority,
            admin: ctx.accounts.admin.key(),
            timestamp: Clock::get()?.unix_timestamp,
        });

        Ok(())
    }

    /// Update oracle authority.
    pub fn update_oracle_authority(
        ctx: Context<UpdateOracleAuthority>,
        new_authority: Pubkey,
    ) -> Result<()> {
        require!(new_authority != Pubkey::default(), LpBondsError::InvalidOracleAuthority);

        let oracle_config = &mut ctx.accounts.oracle_config;
        let old_authority = oracle_config.oracle_authority;
        oracle_config.oracle_authority = new_authority;

        emit!(OracleAuthorityUpdated {
            old_authority,
            new_authority,
            admin: ctx.accounts.admin.key(),
            timestamp: Clock::get()?.unix_timestamp,
        });

        Ok(())
    }

    /// Enable or disable the oracle for the base lp-bonds program.
    /// Matches the pattern used in the evolution program's set_oracle_enabled.
    pub fn set_oracle_enabled(ctx: Context<SetOracleEnabled>, enabled: bool) -> Result<()> {
        ctx.accounts.oracle_config.enabled = enabled;

        emit!(OracleEnabledChanged {
            enabled,
            admin: ctx.accounts.admin.key(),
            timestamp: Clock::get()?.unix_timestamp,
        });

        Ok(())
    }

    /// Initialize nonce account for replay protection.
    ///
    /// SECURITY: Nonce accounts are intentionally NOT closable.
    /// This prevents replay attacks via nonce reset.
    /// Users must reuse the same nonce account permanently.
    /// Uses `init` (not `init_if_needed`) so re-initialization of an
    /// existing account will fail at the Anchor level.
    pub fn initialize_nonce(ctx: Context<InitializeNonce>) -> Result<()> {
        let nonce_account = &mut ctx.accounts.nonce_account;
        nonce_account.user = ctx.accounts.user.key();
        nonce_account.current_nonce = 0;
        nonce_account.bump = ctx.bumps.nonce_account;

        emit!(NonceInitialized {
            user: ctx.accounts.user.key(),
            initial_nonce: 0,
            timestamp: Clock::get()?.unix_timestamp,
        });

        Ok(())
    }

    // =========================================================================
    // EXCHANGE INSTRUCTIONS
    // =========================================================================

    /// Initialize exchange configuration.
    pub fn initialize_exchange_config(
        ctx: Context<InitializeExchangeConfig>,
        token_mint_out: Pubkey,
    ) -> Result<()> {
        require!(token_mint_out != Pubkey::default(), LpBondsError::InvalidExchangeTokenMint);

        let exchange_config = &mut ctx.accounts.exchange_config;
        exchange_config.token_mint_out = token_mint_out;
        exchange_config.is_active = true;
        exchange_config.admin = ctx.accounts.admin.key();
        exchange_config.bump = ctx.bumps.exchange_config;

        emit!(ExchangeConfigInitialized {
            token_mint_out,
            admin: ctx.accounts.admin.key(),
            timestamp: Clock::get()?.unix_timestamp,
        });

        Ok(())
    }

    /// Update exchange configuration (admin only).
    pub fn update_exchange_config(
        ctx: Context<UpdateExchangeConfig>,
        token_mint_out: Pubkey,
        is_active: bool,
    ) -> Result<()> {
        require!(token_mint_out != Pubkey::default(), LpBondsError::InvalidExchangeTokenMint);

        let exchange_config = &mut ctx.accounts.exchange_config;
        exchange_config.token_mint_out = token_mint_out;
        exchange_config.is_active = is_active;

        emit!(ExchangeConfigUpdated {
            token_mint_out,
            is_active,
            admin: ctx.accounts.admin.key(),
            timestamp: Clock::get()?.unix_timestamp,
        });

        Ok(())
    }

    /// Exchange a bond NFT for output tokens (EVM parity).
    ///
    /// Flow (matches LPBondsExchange.sol):
    /// 1. Validate exchange config is active
    /// 2. Validate NFT invariants (decimals=0, supply=1)
    /// 3. Verify oracle signature (domain, bond_mint, amount_out, nonce, sender, program_id)
    /// 4. Create exchange_nonce PDA (replay protection — if exists, nonce already used)
    /// 5. Burn bond NFT
    /// 6. Mint output tokens to user
    pub fn exchange_bonds(
        ctx: Context<ExchangeBonds>,
        amount_out: u64,
        oracle_nonce: u64,
    ) -> Result<()> {
        // --- VALIDATION PHASE ---

        require!(
            ctx.accounts.exchange_config.is_active,
            LpBondsError::ExchangeNotActive
        );

        require!(amount_out > 0, LpBondsError::InvalidAccountData);

        // NFT invariants: bond must be a true NFT (decimals=0, supply=1)
        require!(ctx.accounts.bond_mint.decimals == 0, LpBondsError::InvalidBondMint);
        require!(ctx.accounts.bond_mint.supply == 1, LpBondsError::InvalidBondBalance);

        // Verify exchange_mint_authority PDA is the mint authority for output tokens
        require_keys_eq!(
            ctx.accounts.destination_token_mint.mint_authority.unwrap(),
            ctx.accounts.exchange_mint_authority.key(),
            LpBondsError::InvalidExchangeTokenMint
        );

        // --- ORACLE VERIFICATION ---
        {
            let oracle_config = &ctx.accounts.oracle_config;
            require!(oracle_config.enabled, LpBondsError::OracleNotEnabled);

            let mut domain = [0u8; 18];
            domain.copy_from_slice(ORACLE_DOMAIN_EXCHANGE);
            let expected_message = reconstruct_exchange_message(&ExchangeMessageParams {
                domain,
                bond_mint: ctx.accounts.bond_mint.key(),
                amount_out,
                nonce: oracle_nonce,
                sender: ctx.accounts.user.key(),
                contract_address: crate::ID,
            });

            verify_exchange_attestation(
                &ctx.accounts.instructions_sysvar,
                &oracle_config.oracle_authority,
                &expected_message,
            )?;
        }

        // --- MUTATION PHASE ---

        // Nonce PDA init happens via Anchor `init` constraint on exchange_nonce account.
        // If the PDA already exists, the tx fails (nonce already used).
        let exchange_nonce = &mut ctx.accounts.exchange_nonce;
        exchange_nonce.user = ctx.accounts.user.key();
        exchange_nonce.nonce = oracle_nonce;
        exchange_nonce.bump = ctx.bumps.exchange_nonce;

        // Burn bond NFT
        token::burn(
            CpiContext::new(
                ctx.accounts.token_program.to_account_info(),
                Burn {
                    mint: ctx.accounts.bond_mint.to_account_info(),
                    from: ctx.accounts.user_bond_token_account.to_account_info(),
                    authority: ctx.accounts.user.to_account_info(),
                },
            ),
            1,
        )?;

        // Mint output tokens to user
        let authority_seeds: &[&[u8]] = &[
            EXCHANGE_MINT_AUTHORITY_SEED,
            &[ctx.bumps.exchange_mint_authority],
        ];
        let signer_seeds = &[authority_seeds];

        token::mint_to(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                MintTo {
                    mint: ctx.accounts.destination_token_mint.to_account_info(),
                    to: ctx.accounts.user_destination_token_account.to_account_info(),
                    authority: ctx.accounts.exchange_mint_authority.to_account_info(),
                },
                signer_seeds,
            ),
            amount_out,
        )?;

        emit!(BondExchanged {
            bond_mint: ctx.accounts.bond_mint.key(),
            user: ctx.accounts.user.key(),
            amount_out,
        });

        Ok(())
    }

    /// Emergency token recovery (admin only).
    /// Transfers tokens from a program-controlled account to the admin's token account.
    /// Safety: source account must NOT be a custody position token account.
    pub fn recover_tokens(ctx: Context<RecoverTokens>, amount: u64) -> Result<()> {
        let bond_authority_seeds: &[&[u8]] = &[
            BOND_AUTHORITY_SEED,
            &[ctx.bumps.bond_authority],
        ];
        let signer_seeds = &[bond_authority_seeds];

        token::transfer(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                Transfer {
                    from: ctx.accounts.source_token_account.to_account_info(),
                    to: ctx.accounts.admin_token_account.to_account_info(),
                    authority: ctx.accounts.bond_authority.to_account_info(),
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
    /// token accounts.
    pub fn collect_fees(ctx: Context<CollectFees>) -> Result<()> {
        require!(!ctx.accounts.config.is_paused, LpBondsError::ProtocolPaused);

        // =================================================================
        // TICK ARRAY PDA VALIDATION (defense-in-depth)
        // Matches the validation pattern in add_liquidity_and_mint_bond.
        // =================================================================
        {
            require_keys_eq!(
                ctx.accounts.whirlpool.key(),
                ctx.accounts.position_custody.whirlpool,
                LpBondsError::WhirlpoolNotAllowlisted
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
                LpBondsError::InvalidWhirlpoolProgram
            );
            require_keys_eq!(
                *ctx.accounts.tick_array_upper.owner,
                whirlpool_cpi::WHIRLPOOL_PROGRAM_ID,
                LpBondsError::InvalidWhirlpoolProgram
            );

            let start_lower = whirlpool_cpi::get_start_tick_index(tick_lower, spacing);
            let (expected_lower, _) = whirlpool_cpi::get_tick_array_address(
                &ctx.accounts.whirlpool.key(),
                start_lower,
            );
            require_keys_eq!(
                ctx.accounts.tick_array_lower.key(),
                expected_lower,
                LpBondsError::InvalidTickArrayPda
            );

            let start_upper = whirlpool_cpi::get_start_tick_index(tick_upper, spacing);
            let (expected_upper, _) = whirlpool_cpi::get_tick_array_address(
                &ctx.accounts.whirlpool.key(),
                start_upper,
            );
            require_keys_eq!(
                ctx.accounts.tick_array_upper.key(),
                expected_upper,
                LpBondsError::InvalidTickArrayPda
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

        let bond_mint_key = ctx.accounts.bond_mint.key();
        let custody_seeds: &[&[u8]] = &[
            POSITION_CUSTODY_SEED,
            bond_mint_key.as_ref(),
            &[ctx.accounts.position_custody.bump],
        ];
        let signer_seeds = &[custody_seeds];

        // CPI: collect_fees from the Whirlpool position
        whirlpool_cpi::collect_fees(
            &ctx.accounts.whirlpool_program.to_account_info(),
            &ctx.accounts.whirlpool.to_account_info(),
            &ctx.accounts.position_custody.to_account_info(),
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
    /// meaning the bond NFT has been burned (e.g., during evolution). The
    /// underlying Whirlpool position liquidity remains locked by design; this
    /// instruction only reclaims the PDA rent to the admin.
    pub fn close_orphaned_custody(_ctx: Context<CloseOrphanedCustody>) -> Result<()> {
        // Account is closed via `close = admin` constraint on the account struct.
        // The bond_mint supply == 0 constraint ensures the bond has been burned.
        Ok(())
    }

    /// Verify collateral via oracle signature (post-mint verification).
    ///
    /// Uses the same Ed25519 verification pattern as `add_liquidity_and_mint_bond`:
    /// - Ed25519 instruction must be at (current_index - 1)
    /// - No signature parameter — extracted from Ed25519 instruction
    /// - Strict sequential nonce (current_nonce + 1)
    /// - Timestamp staleness check
    /// - Domain separator ORACLE_DOMAIN_VERIFY prevents cross-instruction replay
    ///
    /// Ticks are read from the position custody record (not user-provided).
    pub fn verify_collateral(
        ctx: Context<VerifyCollateral>,
        amount0: u64,
        amount1: u64,
        liquidity: u128,
        tick_current: i32,
        nonce: u64,
        timestamp: i64,
    ) -> Result<()> {
        let custody = &ctx.accounts.position_custody;
        let now = Clock::get()?.unix_timestamp;

        // Ensure bond_mint and custody are tightly bound — prevents
        // event emission or logic referencing a mismatched custody record.
        require_keys_eq!(
            ctx.accounts.bond_mint.key(),
            custody.bond_mint,
            LpBondsError::InvalidCustodyBondMint
        );

        // =================================================================
        // WHIRLPOOL STATE CROSS-CHECK
        // Verify against the bond's own custody whirlpool and on-chain
        // pool state — NOT the global config (which may have changed
        // since this bond was minted).
        // =================================================================
        // Read token mints from on-chain whirlpool state (bond-specific).
        // Using custody.whirlpool instead of config.token_mint_a/b ensures
        // verify_collateral works correctly even after update_config changes
        // the global pair. Each bond's custody records which whirlpool it
        // was minted against — that is the authoritative source.
        let (whirlpool_mint_a, whirlpool_mint_b) = {
            // Whirlpool::from_account_info checks owner == WHIRLPOOL_PROGRAM_ID
            let whirlpool_state = whirlpool_cpi::Whirlpool::from_account_info(
                &ctx.accounts.whirlpool
            )?;

            // Whirlpool must match the one recorded in position custody
            require_keys_eq!(
                ctx.accounts.whirlpool.key(),
                custody.whirlpool,
                LpBondsError::WhirlpoolNotAllowlisted
            );

            // Oracle-provided tick_current must match on-chain pool state,
            // same invariant enforced in add_liquidity_and_mint_bond.
            require!(
                tick_current == whirlpool_state.tick_current_index,
                LpBondsError::TickCurrentMismatch
            );

            (whirlpool_state.token_mint_a, whirlpool_state.token_mint_b)
        };

        let oracle_config = &ctx.accounts.oracle_config;
        require!(oracle_config.enabled, LpBondsError::OracleNotEnabled);

        // Strict nonce: must be exactly current_nonce + 1
        let expected_nonce = ctx.accounts.nonce_account.current_nonce
            .checked_add(1)
            .ok_or(LpBondsError::ArithmeticOverflow)?;
        require!(nonce == expected_nonce, LpBondsError::InvalidNonceSequence);

        // Timestamp staleness
        let age = now.checked_sub(timestamp)
            .ok_or(LpBondsError::ArithmeticOverflow)?;
        require!(age >= 0, LpBondsError::OracleTimestampFuture);
        require!(age <= MAX_ORACLE_STALENESS_SECONDS, LpBondsError::OracleTimestampStale);

        // Reconstruct message — ticks from custody, mints from whirlpool state.
        // Uses bond-specific whirlpool data, NOT global config, to ensure
        // correct verification after config changes.
        let expected_message = reconstruct_oracle_message(&OracleMessageParams {
            domain: ORACLE_DOMAIN_VERIFY,
            whirlpool: custody.whirlpool,
            token_mint_a: whirlpool_mint_a,
            token_mint_b: whirlpool_mint_b,
            amount_a: amount0,
            amount_b: amount1,
            liquidity,
            tick_lower: custody.tick_lower_index,
            tick_upper: custody.tick_upper_index,
            tick_current,
            nonce,
            timestamp,
            sender: ctx.accounts.sender.key(),
            contract_address: crate::ID,
        });

        // Verify Ed25519 instruction at (current_index - 1)
        verify_oracle_attestation(
            &ctx.accounts.instructions_sysvar,
            &oracle_config.oracle_authority,
            &expected_message,
        )?;

        // Commit nonce
        let old_nonce = ctx.accounts.nonce_account.current_nonce;
        ctx.accounts.nonce_account.current_nonce = nonce;

        emit!(NonceIncremented {
            user: ctx.accounts.sender.key(),
            old_nonce,
            new_nonce: nonce,
            timestamp: now,
        });

        emit!(CollateralVerified {
            bond_mint: ctx.accounts.bond_mint.key(),
            position_mint: custody.position_mint,
            sender: ctx.accounts.sender.key(),
            amount0,
            amount1,
            liquidity,
            nonce,
            oracle_authority: oracle_config.oracle_authority,
            timestamp: now,
        });

        Ok(())
    }
}

// =============================================================================
// ACCOUNT STRUCTS
// =============================================================================

#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,

    #[account(
        init,
        payer = admin,
        space = 8 + ProtocolConfig::INIT_SPACE,
        seeds = [CONFIG_SEED],
        bump
    )]
    pub config: Account<'info, ProtocolConfig>,

    /// CHECK: PDA derived from program
    #[account(seeds = [BOND_AUTHORITY_SEED], bump)]
    pub bond_authority: UncheckedAccount<'info>,

    pub system_program: Program<'info, System>,
}

/// Shared admin-only context for config updates, pause, unpause, propose_admin.
#[derive(Accounts)]
pub struct AdminOnly<'info> {
    #[account(
        constraint = admin.key() == config.admin @ LpBondsError::InvalidAdminAuthority,
    )]
    pub admin: Signer<'info>,

    #[account(
        mut,
        seeds = [CONFIG_SEED],
        bump = config.bump,
    )]
    pub config: Account<'info, ProtocolConfig>,
}

#[derive(Accounts)]
pub struct AcceptAdmin<'info> {
    #[account(
        constraint = new_admin.key() == config.pending_admin @ LpBondsError::InvalidPendingAdmin,
        constraint = config.pending_admin != Pubkey::default() @ LpBondsError::NoPendingAdmin,
    )]
    pub new_admin: Signer<'info>,

    #[account(
        mut,
        seeds = [CONFIG_SEED],
        bump = config.bump,
    )]
    pub config: Account<'info, ProtocolConfig>,
}

#[derive(Accounts)]
#[instruction(
    liquidity_amount: u128,
    token_max_a: u64,
    token_max_b: u64,
    tick_current: i32,
    oracle_nonce: u64,
    oracle_timestamp: i64,
)]
pub struct AddLiquidityAndMintBond<'info> {
    #[account(mut)]
    pub user: Signer<'info>,

    #[account(
        constraint = token_mint_a.key() == config.token_mint_a @ LpBondsError::InvalidTokenMintA,
    )]
    pub token_mint_a: Box<Account<'info, Mint>>,

    #[account(
        constraint = token_mint_b.key() == config.token_mint_b @ LpBondsError::InvalidTokenMintB,
    )]
    pub token_mint_b: Box<Account<'info, Mint>>,

    /// CHECK: PDA validated by seeds
    #[account(seeds = [BOND_AUTHORITY_SEED], bump)]
    pub bond_authority: UncheckedAccount<'info>,

    #[account(
        init,
        payer = user,
        mint::decimals = 0,
        mint::authority = bond_authority,
        mint::freeze_authority = bond_authority,
    )]
    pub bond_mint: Box<Account<'info, Mint>>,

    #[account(
        mut,
        constraint = user_token_a_account.owner == user.key() @ LpBondsError::InvalidTokenOwner,
        constraint = user_token_a_account.mint == token_mint_a.key() @ LpBondsError::InvalidTokenMint,
    )]
    pub user_token_a_account: Box<Account<'info, TokenAccount>>,

    #[account(
        mut,
        constraint = user_token_b_account.owner == user.key() @ LpBondsError::InvalidTokenOwner,
        constraint = user_token_b_account.mint == token_mint_b.key() @ LpBondsError::InvalidTokenMint,
    )]
    pub user_token_b_account: Box<Account<'info, TokenAccount>>,

    #[account(
        init_if_needed,
        payer = user,
        associated_token::mint = bond_mint,
        associated_token::authority = user,
    )]
    pub user_bond_account: Box<Account<'info, TokenAccount>>,

    #[account(
        mut,
        seeds = [CONFIG_SEED],
        bump = config.bump,
        constraint = config.allowlisted_whirlpool == whirlpool.key() @ LpBondsError::WhirlpoolNotAllowlisted,
    )]
    pub config: Box<Account<'info, ProtocolConfig>>,

    #[account(
        init,
        payer = user,
        space = 8 + PositionCustody::INIT_SPACE,
        seeds = [POSITION_CUSTODY_SEED, bond_mint.key().as_ref()],
        bump,
    )]
    pub position_custody: Box<Account<'info, PositionCustody>>,

    /// CHECK: Initialized by Orca Whirlpool CPI
    #[account(mut)]
    pub position_mint: Signer<'info>,

    /// CHECK: Created by Orca Whirlpool CPI
    #[account(mut)]
    pub whirlpool_position: UncheckedAccount<'info>,

    /// CHECK: Created by Orca Whirlpool CPI
    #[account(mut)]
    pub position_token_account: UncheckedAccount<'info>,

    /// CHECK: Created after open_position CPI
    #[account(mut)]
    pub custody_position_token_account: UncheckedAccount<'info>,

    /// CHECK: Validated against config allowlist and owner check in handler
    #[account(
        mut,
        constraint = whirlpool.key() == config.allowlisted_whirlpool @ LpBondsError::WhirlpoolNotAllowlisted,
    )]
    pub whirlpool: UncheckedAccount<'info>,

    /// CHECK: Validated against whirlpool data in handler
    #[account(
        mut,
        constraint = token_vault_a.mint == token_mint_a.key() @ LpBondsError::InvalidTokenMintA,
    )]
    pub token_vault_a: Box<Account<'info, TokenAccount>>,

    /// CHECK: Validated against whirlpool data in handler
    #[account(
        mut,
        constraint = token_vault_b.mint == token_mint_b.key() @ LpBondsError::InvalidTokenMintB,
    )]
    pub token_vault_b: Box<Account<'info, TokenAccount>>,

    /// CHECK: Owner validated against Whirlpool program; PDA derived from
    /// whirlpool address + start_tick_index and verified in handler.
    #[account(mut)]
    pub tick_array_lower: UncheckedAccount<'info>,

    /// CHECK: Owner validated against Whirlpool program; PDA derived from
    /// whirlpool address + start_tick_index and verified in handler.
    #[account(mut)]
    pub tick_array_upper: UncheckedAccount<'info>,

    /// CHECK: Validated against known program ID
    #[account(address = whirlpool_cpi::WHIRLPOOL_PROGRAM_ID @ LpBondsError::InvalidWhirlpoolProgram)]
    pub whirlpool_program: UncheckedAccount<'info>,

    // =========================================================================
    // ORACLE ACCOUNTS — required for mandatory oracle verification
    // =========================================================================

    /// Oracle configuration holding the trusted oracle authority pubkey.
    #[account(
        seeds = [ORACLE_CONFIG_SEED],
        bump = oracle_config.bump,
    )]
    pub oracle_config: Box<Account<'info, OracleConfig>>,

    /// Per-user nonce account for replay protection.
    #[account(
        mut,
        seeds = [NONCE_SEED, user.key().as_ref()],
        bump = nonce_account.bump,
        constraint = nonce_account.user == user.key() @ LpBondsError::InvalidTokenOwner,
    )]
    pub nonce_account: Box<Account<'info, NonceAccount>>,

    /// Instructions sysvar — used to introspect the transaction and verify
    /// the Ed25519SigVerify instruction at (current_index - 1).
    /// CHECK: Validated by address constraint against the known sysvar ID.
    #[account(address = anchor_lang::solana_program::sysvar::instructions::ID)]
    pub instructions_sysvar: AccountInfo<'info>,

    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
    pub rent: Sysvar<'info, Rent>,
}

fn maybe_wrap_native_if_needed<'info>(
    user: &Signer<'info>,
    token_program: &Program<'info, Token>,
    mint: Pubkey,
    user_token_account: &mut Account<'info, TokenAccount>,
    required_amount: u64,
) -> Result<()> {
    if mint != NATIVE_MINT {
        return Ok(());
    }

    token::sync_native(CpiContext::new(
        token_program.to_account_info(),
        SyncNative {
            account: user_token_account.to_account_info(),
        },
    ))?;
    user_token_account.reload()?;

    if user_token_account.amount < required_amount {
        let shortfall = required_amount
            .checked_sub(user_token_account.amount)
            .ok_or(LpBondsError::ArithmeticOverflow)?;

        invoke(
            &system_instruction::transfer(user.key, &user_token_account.key(), shortfall),
            &[
                user.to_account_info(),
                user_token_account.to_account_info(),
            ],
        )?;

        token::sync_native(CpiContext::new(
            token_program.to_account_info(),
            SyncNative {
                account: user_token_account.to_account_info(),
            },
        ))?;
        user_token_account.reload()?;
    }

    Ok(())
}

#[derive(Accounts)]
pub struct RedeemBond<'info> {
    #[account(mut)]
    pub user: Signer<'info>,

    #[account(
        seeds = [CONFIG_SEED],
        bump = config.bump,
    )]
    pub config: Box<Account<'info, ProtocolConfig>>,

    #[account(
        mut,
        constraint = user_bond_account.owner == user.key() @ LpBondsError::InvalidTokenOwner,
        constraint = user_bond_account.mint == bond_mint.key() @ LpBondsError::InvalidBondMint,
        constraint = user_bond_account.amount == 1 @ LpBondsError::InvalidBondBalance,
    )]
    pub user_bond_account: Box<Account<'info, TokenAccount>>,

    #[account(
        init_if_needed,
        payer = user,
        associated_token::mint = position_mint,
        associated_token::authority = user,
    )]
    pub user_position_token_account: Box<Account<'info, TokenAccount>>,

    #[account(mut)]
    pub bond_mint: Box<Account<'info, Mint>>,

    #[account(constraint = position_mint.key() == position_custody.position_mint @ LpBondsError::InvalidPositionMint)]
    pub position_mint: Box<Account<'info, Mint>>,

    #[account(
        mut,
        close = user,
        seeds = [POSITION_CUSTODY_SEED, bond_mint.key().as_ref()],
        bump = position_custody.bump,
        constraint = position_custody.bond_mint == bond_mint.key() @ LpBondsError::InvalidCustodyBondMint,
    )]
    pub position_custody: Box<Account<'info, PositionCustody>>,

    #[account(
        mut,
        constraint = custody_position_token_account.owner == position_custody.key() @ LpBondsError::InvalidTokenOwner,
        constraint = custody_position_token_account.mint == position_mint.key() @ LpBondsError::InvalidPositionMint,
        constraint = custody_position_token_account.amount == 1 @ LpBondsError::PositionNftNotInCustody,
    )]
    pub custody_position_token_account: Box<Account<'info, TokenAccount>>,

    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct InitializeOracle<'info> {
    #[account(
        mut,
        constraint = admin.key() == config.admin @ LpBondsError::InvalidAdminAuthority,
    )]
    pub admin: Signer<'info>,

    #[account(seeds = [CONFIG_SEED], bump = config.bump)]
    pub config: Account<'info, ProtocolConfig>,

    #[account(
        init,
        payer = admin,
        space = 8 + OracleConfig::INIT_SPACE,
        seeds = [ORACLE_CONFIG_SEED],
        bump,
    )]
    pub oracle_config: Account<'info, OracleConfig>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct UpdateOracleAuthority<'info> {
    #[account(constraint = admin.key() == config.admin @ LpBondsError::InvalidAdminAuthority)]
    pub admin: Signer<'info>,

    #[account(seeds = [CONFIG_SEED], bump = config.bump)]
    pub config: Account<'info, ProtocolConfig>,

    #[account(mut, seeds = [ORACLE_CONFIG_SEED], bump = oracle_config.bump)]
    pub oracle_config: Account<'info, OracleConfig>,
}

#[derive(Accounts)]
pub struct SetOracleEnabled<'info> {
    #[account(constraint = admin.key() == config.admin @ LpBondsError::InvalidAdminAuthority)]
    pub admin: Signer<'info>,

    #[account(seeds = [CONFIG_SEED], bump = config.bump)]
    pub config: Account<'info, ProtocolConfig>,

    #[account(mut, seeds = [ORACLE_CONFIG_SEED], bump = oracle_config.bump)]
    pub oracle_config: Account<'info, OracleConfig>,
}

#[derive(Accounts)]
pub struct InitializeNonce<'info> {
    #[account(mut)]
    pub user: Signer<'info>,

    #[account(
        init,
        payer = user,
        space = 8 + NonceAccount::INIT_SPACE,
        seeds = [NONCE_SEED, user.key().as_ref()],
        bump,
    )]
    pub nonce_account: Account<'info, NonceAccount>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(
    amount0: u64,
    amount1: u64,
    liquidity: u128,
    tick_current: i32,
    nonce: u64,
    timestamp: i64,
)]
pub struct VerifyCollateral<'info> {
    pub sender: Signer<'info>,

    #[account(seeds = [ORACLE_CONFIG_SEED], bump = oracle_config.bump)]
    pub oracle_config: Account<'info, OracleConfig>,

    #[account(seeds = [CONFIG_SEED], bump = config.bump)]
    pub config: Account<'info, ProtocolConfig>,

    #[account(
        mut,
        seeds = [NONCE_SEED, sender.key().as_ref()],
        bump = nonce_account.bump,
        constraint = nonce_account.user == sender.key() @ LpBondsError::InvalidTokenOwner,
    )]
    pub nonce_account: Account<'info, NonceAccount>,

    pub bond_mint: Account<'info, Mint>,

    /// Verify the sender actually owns the bond NFT.
    #[account(
        constraint = sender_bond_account.owner == sender.key() @ LpBondsError::InvalidTokenOwner,
        constraint = sender_bond_account.mint == bond_mint.key() @ LpBondsError::InvalidBondMint,
        constraint = sender_bond_account.amount == 1 @ LpBondsError::InvalidBondBalance,
    )]
    pub sender_bond_account: Account<'info, TokenAccount>,

    #[account(
        seeds = [POSITION_CUSTODY_SEED, bond_mint.key().as_ref()],
        bump = position_custody.bump,
        constraint = position_custody.bond_mint == bond_mint.key() @ LpBondsError::InvalidCustodyBondMint,
    )]
    pub position_custody: Account<'info, PositionCustody>,

    /// CHECK: Deserialized and validated via Whirlpool::from_account_info
    /// in handler (checks owner == WHIRLPOOL_PROGRAM_ID). Key validated
    /// against position_custody.whirlpool in handler.
    pub whirlpool: AccountInfo<'info>,

    /// CHECK: Validated by address constraint
    #[account(address = anchor_lang::solana_program::sysvar::instructions::ID)]
    pub instructions_sysvar: AccountInfo<'info>,
}

#[derive(Accounts)]
pub struct InitializeExchangeConfig<'info> {
    #[account(
        mut,
        constraint = admin.key() == config.admin @ LpBondsError::InvalidAdminAuthority,
    )]
    pub admin: Signer<'info>,

    #[account(seeds = [CONFIG_SEED], bump = config.bump)]
    pub config: Account<'info, ProtocolConfig>,

    #[account(
        init,
        payer = admin,
        space = 8 + ExchangeConfig::INIT_SPACE,
        seeds = [EXCHANGE_CONFIG_SEED],
        bump,
    )]
    pub exchange_config: Account<'info, ExchangeConfig>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct UpdateExchangeConfig<'info> {
    #[account(
        constraint = admin.key() == config.admin @ LpBondsError::InvalidAdminAuthority,
    )]
    pub admin: Signer<'info>,

    #[account(seeds = [CONFIG_SEED], bump = config.bump)]
    pub config: Account<'info, ProtocolConfig>,

    #[account(
        mut,
        seeds = [EXCHANGE_CONFIG_SEED],
        bump = exchange_config.bump,
    )]
    pub exchange_config: Account<'info, ExchangeConfig>,
}

#[derive(Accounts)]
#[instruction(amount_out: u64, oracle_nonce: u64)]
pub struct ExchangeBonds<'info> {
    #[account(mut)]
    pub user: Signer<'info>,

    #[account(
        seeds = [EXCHANGE_CONFIG_SEED],
        bump = exchange_config.bump,
    )]
    pub exchange_config: Box<Account<'info, ExchangeConfig>>,

    #[account(
        seeds = [ORACLE_CONFIG_SEED],
        bump = oracle_config.bump,
    )]
    pub oracle_config: Box<Account<'info, OracleConfig>>,

    /// PDA-per-nonce replay protection.
    /// If this PDA already exists, the nonce has been used and init will fail.
    #[account(
        init,
        payer = user,
        space = 8 + ExchangeNonce::INIT_SPACE,
        seeds = [EXCHANGE_NONCE_SEED, user.key().as_ref(), &oracle_nonce.to_le_bytes()],
        bump,
    )]
    pub exchange_nonce: Box<Account<'info, ExchangeNonce>>,

    /// Bond NFT mint to be burned.
    #[account(mut)]
    pub bond_mint: Box<Account<'info, Mint>>,

    /// User's bond token account (must hold exactly 1).
    #[account(
        mut,
        constraint = user_bond_token_account.owner == user.key() @ LpBondsError::InvalidTokenOwner,
        constraint = user_bond_token_account.mint == bond_mint.key() @ LpBondsError::InvalidBondMint,
        constraint = user_bond_token_account.amount == 1 @ LpBondsError::InvalidBondBalance,
    )]
    pub user_bond_token_account: Box<Account<'info, TokenAccount>>,

    /// Output SPL token mint. Must match exchange_config.token_mint_out.
    /// The exchange_mint_authority PDA must be the mint authority.
    #[account(
        mut,
        constraint = destination_token_mint.key() == exchange_config.token_mint_out @ LpBondsError::InvalidExchangeTokenMint,
    )]
    pub destination_token_mint: Box<Account<'info, Mint>>,

    /// User's destination token account for output tokens.
    #[account(
        mut,
        constraint = user_destination_token_account.owner == user.key() @ LpBondsError::InvalidTokenOwner,
        constraint = user_destination_token_account.mint == destination_token_mint.key() @ LpBondsError::InvalidTokenMint,
    )]
    pub user_destination_token_account: Box<Account<'info, TokenAccount>>,

    /// CHECK: PDA used as mint authority for output tokens.
    #[account(seeds = [EXCHANGE_MINT_AUTHORITY_SEED], bump)]
    pub exchange_mint_authority: UncheckedAccount<'info>,

    /// CHECK: Validated by address constraint
    #[account(address = anchor_lang::solana_program::sysvar::instructions::ID)]
    pub instructions_sysvar: AccountInfo<'info>,

    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct RecoverTokens<'info> {
    #[account(
        mut,
        constraint = admin.key() == config.admin @ LpBondsError::InvalidAdminAuthority,
    )]
    pub admin: Signer<'info>,

    #[account(seeds = [CONFIG_SEED], bump = config.bump)]
    pub config: Account<'info, ProtocolConfig>,

    /// CHECK: PDA used as signer for transfer
    #[account(seeds = [BOND_AUTHORITY_SEED], bump)]
    pub bond_authority: UncheckedAccount<'info>,

    /// Source token account controlled by the program (owned by bond_authority PDA).
    /// Bound to position_custody via mint == position_custody.position_mint.
    #[account(
        mut,
        constraint = source_token_account.owner == bond_authority.key() @ LpBondsError::InvalidTokenOwner,
        constraint = source_token_account.mint == position_custody.position_mint @ LpBondsError::InvalidPositionMint,
    )]
    pub source_token_account: Account<'info, TokenAccount>,

    /// Bond mint associated with the custody being recovered.
    /// Must have supply == 0, proving the bond has been burned and the position
    /// is no longer active. This prevents draining custody position token accounts
    /// that hold active bond's position NFTs.
    #[account(constraint = bond_mint.supply == 0 @ LpBondsError::RecoveryCustodyProtected)]
    pub bond_mint: Account<'info, Mint>,

    /// PositionCustody PDA that binds bond_mint to position_mint.
    /// Ensures the supply == 0 check on bond_mint applies to the actual
    /// position whose tokens are being recovered — prevents passing an
    /// unrelated burned mint to bypass the check.
    #[account(
        seeds = [POSITION_CUSTODY_SEED, bond_mint.key().as_ref()],
        bump = position_custody.bump,
        constraint = position_custody.bond_mint == bond_mint.key() @ LpBondsError::InvalidCustodyBondMint,
    )]
    pub position_custody: Account<'info, PositionCustody>,

    /// Admin's token account to receive recovered tokens.
    #[account(mut)]
    pub admin_token_account: Account<'info, TokenAccount>,

    pub token_program: Program<'info, Token>,
}

#[derive(Accounts)]
pub struct CloseOrphanedCustody<'info> {
    #[account(
        mut,
        constraint = admin.key() == config.admin @ LpBondsError::InvalidAdminAuthority,
    )]
    pub admin: Signer<'info>,

    #[account(seeds = [CONFIG_SEED], bump = config.bump)]
    pub config: Account<'info, ProtocolConfig>,

    /// The bond mint associated with the custody. Must have supply == 0
    /// (the bond NFT has been burned).
    #[account(constraint = bond_mint.supply == 0 @ LpBondsError::InvalidBondBalance)]
    pub bond_mint: Account<'info, Mint>,

    /// The orphaned PositionCustody PDA to close. Rent is returned to admin.
    #[account(
        mut,
        close = admin,
        seeds = [POSITION_CUSTODY_SEED, bond_mint.key().as_ref()],
        bump = position_custody.bump,
        constraint = position_custody.bond_mint == bond_mint.key() @ LpBondsError::InvalidCustodyBondMint,
    )]
    pub position_custody: Account<'info, PositionCustody>,
}

#[derive(Accounts)]
pub struct CollectFees<'info> {
    #[account(mut)]
    pub user: Signer<'info>,

    #[account(
        seeds = [CONFIG_SEED],
        bump = config.bump,
    )]
    pub config: Box<Account<'info, ProtocolConfig>>,

    /// User must hold exactly 1 bond NFT to collect fees.
    #[account(
        constraint = user_bond_account.owner == user.key() @ LpBondsError::InvalidTokenOwner,
        constraint = user_bond_account.mint == bond_mint.key() @ LpBondsError::InvalidBondMint,
        constraint = user_bond_account.amount == 1 @ LpBondsError::InvalidBondBalance,
    )]
    pub user_bond_account: Box<Account<'info, TokenAccount>>,

    pub bond_mint: Box<Account<'info, Mint>>,

    #[account(constraint = position_mint.key() == position_custody.position_mint @ LpBondsError::InvalidPositionMint)]
    pub position_mint: Box<Account<'info, Mint>>,

    #[account(
        seeds = [POSITION_CUSTODY_SEED, bond_mint.key().as_ref()],
        bump = position_custody.bump,
        constraint = position_custody.bond_mint == bond_mint.key() @ LpBondsError::InvalidCustodyBondMint,
    )]
    pub position_custody: Box<Account<'info, PositionCustody>>,

    #[account(
        constraint = custody_position_token_account.owner == position_custody.key() @ LpBondsError::InvalidTokenOwner,
        constraint = custody_position_token_account.mint == position_mint.key() @ LpBondsError::InvalidPositionMint,
        constraint = custody_position_token_account.amount == 1 @ LpBondsError::PositionNftNotInCustody,
    )]
    pub custody_position_token_account: Box<Account<'info, TokenAccount>>,

    /// CHECK: Whirlpool position account (Orca PDA).
    #[account(mut)]
    pub whirlpool_position: UncheckedAccount<'info>,

    /// CHECK: Validated against custody whirlpool in handler
    #[account(
        constraint = whirlpool.key() == position_custody.whirlpool @ LpBondsError::WhirlpoolNotAllowlisted,
    )]
    pub whirlpool: UncheckedAccount<'info>,

    /// User's token A account to receive fees.
    #[account(
        mut,
        constraint = user_token_a_account.owner == user.key() @ LpBondsError::InvalidTokenOwner,
    )]
    pub user_token_a_account: Box<Account<'info, TokenAccount>>,

    /// User's token B account to receive fees.
    #[account(
        mut,
        constraint = user_token_b_account.owner == user.key() @ LpBondsError::InvalidTokenOwner,
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
    #[account(address = whirlpool_cpi::WHIRLPOOL_PROGRAM_ID @ LpBondsError::InvalidWhirlpoolProgram)]
    pub whirlpool_program: UncheckedAccount<'info>,

    pub token_program: Program<'info, Token>,
}
