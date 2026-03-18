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
/// - Configurable admin, pause, whirlpool, and token settings
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
    pub fn initialize(
        ctx: Context<Initialize>,
        whirlpool: Pubkey,
        token_mint_a: Pubkey,
        token_mint_b: Pubkey,
        lock_duration: i64,
    ) -> Result<()> {
        let config = &mut ctx.accounts.config;
        config.admin = ctx.accounts.admin.key();
        config.pending_admin = Pubkey::default();
        config.allowlisted_whirlpool = whirlpool;
        config.token_mint_a = token_mint_a;
        config.token_mint_b = token_mint_b;
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
        lock_duration: i64,
    ) -> Result<()> {
        let config = &mut ctx.accounts.config;
        config.allowlisted_whirlpool = whirlpool;
        config.token_mint_a = token_mint_a;
        config.token_mint_b = token_mint_b;
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
    /// Supports any Whirlpool token pair (SOL, stablecoins, arbitrary SPL tokens).
    /// Token mints and vaults are sourced from and validated against on-chain Whirlpool state.
    /// Native SOL wrapping is performed only when one of the Whirlpool token mints is NATIVE_MINT.
    pub fn add_liquidity_and_mint_bond(
        ctx: Context<AddLiquidityAndMintBond>,
        tick_lower_index: i32,
        tick_upper_index: i32,
        liquidity_amount: u128,
        token_max_a: u64,
        token_max_b: u64,
    ) -> Result<()> {
        require!(!ctx.accounts.config.is_paused, LpBondsError::ProtocolPaused);
        require!(tick_lower_index < tick_upper_index, LpBondsError::InvalidTickRange);
        require!(tick_lower_index >= MIN_TICK_INDEX, LpBondsError::TickOutOfBounds);
        require!(tick_upper_index <= MAX_TICK_INDEX, LpBondsError::TickOutOfBounds);

        let now = Clock::get()?.unix_timestamp;

        // Validate token mints and vaults against whirlpool on-chain data
        {
            // Fix: verify whirlpool account is owned by the Whirlpool program before deserializing
            require_keys_eq!(
                *ctx.accounts.whirlpool.owner,
                ctx.accounts.whirlpool_program.key(),
                LpBondsError::InvalidWhirlpoolProgram
            );

            let whirlpool_state = whirlpool_cpi::Whirlpool::from_account_info(
                &ctx.accounts.whirlpool
            )?;

            // Validate token mints match config and passed accounts
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

            // Validate token vaults match whirlpool state
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

            // Validate user token accounts against whirlpool token mints
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

            // Validate vault token account mints against whirlpool token mints
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
        }

        // Only wrap native SOL when a whirlpool side uses NATIVE_MINT.
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

        // STEP 1: Open Whirlpool position via CPI
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

        // Fix: validate position_token_account was initialized by open_position with the correct mint
        {
            let pta_data = ctx.accounts.position_token_account.data.borrow();
            let pta = anchor_spl::token::TokenAccount::try_deserialize(&mut &pta_data[..])?;
            require_keys_eq!(
                pta.mint,
                ctx.accounts.position_mint.key(),
                LpBondsError::InvalidTokenMint
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

        Ok(())
    }

    /// Redeem bond NFT to reclaim the underlying position.
    pub fn redeem_bond(ctx: Context<RedeemBond>) -> Result<()> {
        require!(!ctx.accounts.config.is_paused, LpBondsError::ProtocolPaused);

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

    /// Initialize nonce account for replay protection.
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

    /// Verify collateral via oracle signature.
    pub fn verify_collateral(
        ctx: Context<VerifyCollateral>,
        amount0: u64,
        amount1: u64,
        liquidity: u128,
        tick_lower: i32,
        tick_upper: i32,
        tick_current: i32,
        nonce: u64,
        signature: [u8; 64],
    ) -> Result<()> {
        let custody = &ctx.accounts.position_custody;
        
        // Validate position data
        require!(
            tick_lower == custody.tick_lower_index,
            LpBondsError::PositionDataMismatch
        );
        require!(
            tick_upper == custody.tick_upper_index,
            LpBondsError::PositionDataMismatch
        );

        // Validate nonce
        let current_nonce = ctx.accounts.nonce_account.current_nonce;
        require!(nonce > current_nonce, LpBondsError::NonceAlreadyUsed);

        // Reconstruct and verify signature
        let canonical_params = CanonicalMessageParams {
            bond_mint: ctx.accounts.bond_mint.key(),
            position_mint: custody.position_mint,
            amount0,
            amount1,
            liquidity,
            tick_lower,
            tick_upper,
            tick_current,
            nonce,
            sender: ctx.accounts.sender.key(),
            contract_address: crate::ID,
        };

        let expected_message = reconstruct_canonical_message(&canonical_params);

        verify_ed25519_instruction(
            &ctx.accounts.instructions_sysvar,
            &ctx.accounts.oracle_config.oracle_authority,
            &signature,
            &expected_message,
        )?;

        // Update nonce
        let old_nonce = ctx.accounts.nonce_account.current_nonce;
        let nonce_account = &mut ctx.accounts.nonce_account;
        nonce_account.current_nonce = nonce;

        emit!(NonceIncremented {
            user: ctx.accounts.sender.key(),
            old_nonce,
            new_nonce: nonce,
            timestamp: Clock::get()?.unix_timestamp,
        });

        emit!(CollateralVerified {
            bond_mint: ctx.accounts.bond_mint.key(),
            position_mint: custody.position_mint,
            sender: ctx.accounts.sender.key(),
            amount0,
            amount1,
            liquidity,
            nonce,
            oracle_authority: ctx.accounts.oracle_config.oracle_authority,
            timestamp: Clock::get()?.unix_timestamp,
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
    tick_lower_index: i32,
    tick_upper_index: i32,
    liquidity_amount: u128,
    token_max_a: u64,
    token_max_b: u64,
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

    /// CHECK: Validated by Orca Whirlpool program
    #[account(mut)]
    pub tick_array_lower: UncheckedAccount<'info>,

    /// CHECK: Validated by Orca Whirlpool program
    #[account(mut)]
    pub tick_array_upper: UncheckedAccount<'info>,

    /// CHECK: Validated against known program ID
    #[account(address = whirlpool_cpi::WHIRLPOOL_PROGRAM_ID @ LpBondsError::InvalidWhirlpoolProgram)]
    pub whirlpool_program: UncheckedAccount<'info>,

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
    pub config: Account<'info, ProtocolConfig>,

    #[account(
        mut,
        constraint = user_bond_account.owner == user.key() @ LpBondsError::InvalidTokenOwner,
        constraint = user_bond_account.mint == bond_mint.key() @ LpBondsError::InvalidBondMint,
        constraint = user_bond_account.amount == 1 @ LpBondsError::InvalidBondBalance,
    )]
    pub user_bond_account: Account<'info, TokenAccount>,

    #[account(
        init_if_needed,
        payer = user,
        associated_token::mint = position_mint,
        associated_token::authority = user,
    )]
    pub user_position_token_account: Account<'info, TokenAccount>,

    #[account(mut)]
    pub bond_mint: Account<'info, Mint>,

    #[account(constraint = position_mint.key() == position_custody.position_mint @ LpBondsError::InvalidPositionMint)]
    pub position_mint: Account<'info, Mint>,

    #[account(
        mut,
        seeds = [POSITION_CUSTODY_SEED, bond_mint.key().as_ref()],
        bump = position_custody.bump,
        constraint = position_custody.bond_mint == bond_mint.key() @ LpBondsError::InvalidCustodyBondMint,
    )]
    pub position_custody: Account<'info, PositionCustody>,

    #[account(
        mut,
        constraint = custody_position_token_account.owner == position_custody.key() @ LpBondsError::InvalidTokenOwner,
        constraint = custody_position_token_account.mint == position_mint.key() @ LpBondsError::InvalidPositionMint,
        constraint = custody_position_token_account.amount == 1 @ LpBondsError::PositionNftNotInCustody,
    )]
    pub custody_position_token_account: Account<'info, TokenAccount>,

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
    #[account(constraint = admin.key() == oracle_config.admin @ LpBondsError::InvalidAdminAuthority)]
    pub admin: Signer<'info>,

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
    tick_lower: i32,
    tick_upper: i32,
    tick_current: i32,
    nonce: u64,
    signature: [u8; 64],
)]
pub struct VerifyCollateral<'info> {
    pub sender: Signer<'info>,

    #[account(seeds = [ORACLE_CONFIG_SEED], bump = oracle_config.bump)]
    pub oracle_config: Account<'info, OracleConfig>,

    #[account(
        mut,
        seeds = [NONCE_SEED, sender.key().as_ref()],
        bump = nonce_account.bump,
        constraint = nonce_account.user == sender.key() @ LpBondsError::InvalidTokenOwner,
    )]
    pub nonce_account: Account<'info, NonceAccount>,

    pub bond_mint: Account<'info, Mint>,

    #[account(
        seeds = [POSITION_CUSTODY_SEED, bond_mint.key().as_ref()],
        bump = position_custody.bump,
        constraint = position_custody.bond_mint == bond_mint.key() @ LpBondsError::InvalidCustodyBondMint,
    )]
    pub position_custody: Account<'info, PositionCustody>,

    /// CHECK: Validated by solana_program
    #[account(address = anchor_lang::solana_program::sysvar::instructions::ID)]
    pub instructions_sysvar: AccountInfo<'info>,
}
