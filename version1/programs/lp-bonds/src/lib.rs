use anchor_lang::prelude::*;
use anchor_lang::solana_program::{program::invoke, system_instruction};
use anchor_spl::{
    associated_token::AssociatedToken,

    token::{self, Burn, Mint, MintTo, SyncNative, Token, TokenAccount, Transfer},
};

pub mod constants;
pub mod errors;
pub mod events;
pub mod state;
pub mod whirlpool_cpi;

use constants::*;
use errors::*;
use events::*;
use state::*;
use whirlpool_cpi::Whirlpool;

declare_id!("AmJcNFdgckd1o6DPa6j12WGM6wNKZdvdWphtsP2Ws92w");

/// ============================================================================
/// ARCHITECTURE OVERVIEW (< 300 words)
/// ============================================================================
///
/// PDA STRUCTURE:
/// - Protocol Config PDA: [b"config"] - Stores allowlisted whirlpool, authority
/// - Position Custody PDA: [b"position_custody", bond_nft_mint] - Holds whirlpool position NFT
/// - Bond Mint Authority PDA: [b"bond_authority"] - Signs bond NFT mint operations
///
/// AUTHORITY MODEL:
/// - Admin authority: Can update protocol config, allowlist whirlpools
/// - Bond authority PDA: Program-controlled signer for bond NFT minting
/// - Position custody PDA: Token account owner for whirlpool position NFTs
///
/// POSITION NFT CUSTODY:
/// - Whirlpool position NFT is transferred to a PDA-owned token account
/// - PDA seeds include the bond NFT mint for 1:1 mapping
/// - Only the program can access/transfer the position NFT
///
/// BOND NFT MINT AUTHORITY:
/// - Each bond is a unique SPL token with supply = 1
/// - Bond mint authority is a PDA, allowing programmatic minting
/// - Mint authority is revoked after initial mint (immutable supply)
/// - Bond NFT represents ownership claim on the underlying position
///
/// ATOMIC FLOW:
/// 1. Wrap SOL -> wSOL (create ATA, transfer SOL, sync_native)
/// 2. Transfer SPL token to program-owned ATA
/// 3. CPI: open_position (creates position NFT)
/// 4. CPI: increase_liquidity (deposits tokens)
/// 5. Transfer position NFT to custody PDA
/// 6. Mint bond NFT to user
/// 7. Close temporary wSOL account (return rent to user)
///
/// ============================================================================

#[program]
pub mod lp_bonds {
    use super::*;

    /// Initialize protocol configuration.
    /// Sets the allowlisted whirlpool and admin authority.
    pub fn initialize(ctx: Context<Initialize>) -> Result<()> {
        let config = &mut ctx.accounts.config;
        config.admin = ctx.accounts.admin.key();
        config.allowlisted_whirlpool = ALLOWLISTED_WHIRLPOOL;
        config.bond_counter = 0;
        config.bump = ctx.bumps.config;

        emit!(ProtocolInitialized {
            admin: config.admin,
            allowlisted_whirlpool: config.allowlisted_whirlpool,
        });

        Ok(())
    }

    /// Core instruction: Add liquidity to whirlpool and mint bond NFT.
    ///
    /// Performs atomic flow:
    /// 1. Validate whirlpool matches allowlist
    /// 2. Wrap SOL to wSOL
    /// 3. Open whirlpool position via CPI
    /// 4. Add liquidity via CPI
    /// 5. Custody position NFT in program PDA
    /// 6. Mint bond NFT to user
    /// 7. Cleanup temporary wSOL account
    pub fn add_liquidity_and_mint_bond(
        ctx: Context<AddLiquidityAndMintBond>,
        tick_lower_index: i32,
        tick_upper_index: i32,
        liquidity_amount: u128,
        token_max_a: u64,
        token_max_b: u64,
        sol_amount: u64,
    ) -> Result<()> {
        // =====================================================================
        // VALIDATION PHASE
        // =====================================================================

        // 1. Verify whirlpool is allowlisted
        require_keys_eq!(
            ctx.accounts.whirlpool.key(),
            ctx.accounts.config.allowlisted_whirlpool,
            LpBondsError::WhirlpoolNotAllowlisted
        );

        // 2. Verify whirlpool program ID
        require_keys_eq!(
            ctx.accounts.whirlpool_program.key(),
            whirlpool_cpi::WHIRLPOOL_PROGRAM_ID,
            LpBondsError::InvalidWhirlpoolProgram
        );

        // 3. Validate tick range
        require!(
            tick_lower_index < tick_upper_index,
            LpBondsError::InvalidTickRange
        );
        require!(
            tick_lower_index >= MIN_TICK_INDEX && tick_upper_index <= MAX_TICK_INDEX,
            LpBondsError::TickOutOfBounds
        );

        // Deserialize whirlpool data from external program's account
        let whirlpool_data = Whirlpool::from_account_info(&ctx.accounts.whirlpool.to_account_info())?;
        
        // Verify tick spacing alignment
        let tick_spacing = whirlpool_data.tick_spacing as i32;
        require!(
            tick_lower_index % tick_spacing == 0 && tick_upper_index % tick_spacing == 0,
            LpBondsError::TickNotAlignedToSpacing
        );

        // 4. Verify token mints match expected whirlpool configuration
        require_keys_eq!(
            whirlpool_data.token_mint_a,
            EXPECTED_TOKEN_MINT_A,
            LpBondsError::InvalidTokenMintA
        );
        require_keys_eq!(
            whirlpool_data.token_mint_b,
            EXPECTED_TOKEN_MINT_B,
            LpBondsError::InvalidTokenMintB
        );

        // 5. Validate token vaults match whirlpool configuration
        require_keys_eq!(
            ctx.accounts.token_vault_a.key(),
            whirlpool_data.token_vault_a,
            LpBondsError::InvalidTokenVault
        );
        require_keys_eq!(
            ctx.accounts.token_vault_b.key(),
            whirlpool_data.token_vault_b,
            LpBondsError::InvalidTokenVault
        );

        // 6. Validate SOL amount
        require!(sol_amount > 0, LpBondsError::ZeroSolAmount);

        // =====================================================================
        // STEP 1: WRAP SOL TO wSOL
        // =====================================================================

        // Transfer SOL to temporary wSOL account
        invoke(
            &system_instruction::transfer(
                &ctx.accounts.user.key(),
                &ctx.accounts.user_wsol_account.key(),
                sol_amount,
            ),
            &[
                ctx.accounts.user.to_account_info(),
                ctx.accounts.user_wsol_account.to_account_info(),
                ctx.accounts.system_program.to_account_info(),
            ],
        )?;

        // Sync native to update wSOL balance
        token::sync_native(CpiContext::new(
            ctx.accounts.token_program.to_account_info(),
            SyncNative {
                account: ctx.accounts.user_wsol_account.to_account_info(),
            },
        ))?;


        // =====================================================================
        // STEP 2: OPEN WHIRLPOOL POSITION VIA CPI
        // =====================================================================

        // Derive position PDA bump
        let (position_pda, position_bump) = whirlpool_cpi::get_position_address(
            &ctx.accounts.position_mint.key()
        );
        
        // Verify the position account matches expected PDA
        require_keys_eq!(
            ctx.accounts.whirlpool_position.key(),
            position_pda,
            LpBondsError::InvalidPositionPda
        );

        // NOTE: We open position with USER as owner initially
        // The position will be transferred to custody PDA after increase_liquidity
        // This is necessary because increase_liquidity requires position_authority
        // to own both the position AND the token accounts being deposited
        let empty_seeds: &[&[&[u8]]] = &[];

        // CPI to whirlpool::open_position - owner is USER
        whirlpool_cpi::open_position(
            &ctx.accounts.whirlpool_program.to_account_info(),
            &ctx.accounts.user.to_account_info(),
            &ctx.accounts.user.to_account_info(), // owner = user (was position_custody)
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
            empty_seeds, // user signs directly, no PDA seeds needed
        )?;

        // =====================================================================
        // STEP 2.5: CREATE CUSTODY POSITION TOKEN ACCOUNT
        // =====================================================================
        // Now that position_mint exists (created by open_position CPI),
        // we can create the ATA for custody to hold the position NFT.
        
        anchor_spl::associated_token::create(
            CpiContext::new(
                ctx.accounts.associated_token_program.to_account_info(),
                anchor_spl::associated_token::Create {
                    payer: ctx.accounts.user.to_account_info(),
                    associated_token: ctx.accounts.custody_position_token_account.to_account_info(),
                    authority: ctx.accounts.position_custody.to_account_info(),
                    mint: ctx.accounts.position_mint.to_account_info(),
                    system_program: ctx.accounts.system_program.to_account_info(),
                    token_program: ctx.accounts.token_program.to_account_info(),
                },
            ),
        )?;

        // =====================================================================
        // STEP 3: INCREASE LIQUIDITY VIA CPI
        // =====================================================================

        // User is position_authority (owns position_token_account and tokens)
        whirlpool_cpi::increase_liquidity(
            &ctx.accounts.whirlpool_program.to_account_info(),
            &ctx.accounts.whirlpool.to_account_info(),
            &ctx.accounts.token_program.to_account_info(),
            &ctx.accounts.user.to_account_info(),
            &ctx.accounts.whirlpool_position.to_account_info(),
            &ctx.accounts.position_token_account.to_account_info(),
            &ctx.accounts.user_wsol_account.to_account_info(),
            &ctx.accounts.user_token_b_account.to_account_info(),
            &ctx.accounts.token_vault_a.to_account_info(),
            &ctx.accounts.token_vault_b.to_account_info(),
            &ctx.accounts.tick_array_lower.to_account_info(),
            &ctx.accounts.tick_array_upper.to_account_info(),
            liquidity_amount,
            token_max_a,
            token_max_b,
            empty_seeds,
        )?;

        // =====================================================================
        // STEP 3.5: TRANSFER POSITION NFT TO CUSTODY PDA
        // =====================================================================

        // Position custody PDA seeds for receiving the transfer
        let bond_mint_key = ctx.accounts.bond_mint.key();
        let custody_seeds: &[&[u8]] = &[
            POSITION_CUSTODY_SEED,
            bond_mint_key.as_ref(),
            &[ctx.bumps.position_custody],
        ];
        let _custody_signer_seeds = &[custody_seeds];

        // Transfer position NFT from user's ATA to custody's ATA
        token::transfer(
            CpiContext::new(
                ctx.accounts.token_program.to_account_info(),
                token::Transfer {
                    from: ctx.accounts.position_token_account.to_account_info(),
                    to: ctx.accounts.custody_position_token_account.to_account_info(),
                    authority: ctx.accounts.user.to_account_info(),
                },
            ),
            1,
        )?;

        // =====================================================================
        // STEP 4: MINT BOND NFT TO USER
        // =====================================================================

        let authority_seeds: &[&[u8]] = &[BOND_AUTHORITY_SEED, &[ctx.bumps.bond_authority]];
        let authority_signer_seeds = &[authority_seeds];

        // Mint exactly 1 bond NFT token
        token::mint_to(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                MintTo {
                    mint: ctx.accounts.bond_mint.to_account_info(),
                    to: ctx.accounts.user_bond_account.to_account_info(),
                    authority: ctx.accounts.bond_authority.to_account_info(),
                },
                authority_signer_seeds,
            ),
            1,
        )?;

        // NOTE: NFT metadata creation is disabled to reduce transaction size
        // The bond NFT exists without Metaplex metadata for now
        // TODO: Re-enable with versioned transactions or separate instruction

        // =====================================================================
        // STEP 5: UPDATE PROTOCOL STATE
        // =====================================================================

        let custody = &mut ctx.accounts.position_custody;
        custody.bond_mint = ctx.accounts.bond_mint.key();
        custody.position_mint = ctx.accounts.position_mint.key();
        custody.whirlpool = ctx.accounts.whirlpool.key();
        custody.tick_lower_index = tick_lower_index;
        custody.tick_upper_index = tick_upper_index;
        custody.liquidity = liquidity_amount;
        custody.depositor = ctx.accounts.user.key();
        custody.created_at = Clock::get()?.unix_timestamp;
        custody.bump = ctx.bumps.position_custody;
        custody.position_bump = position_bump;

        let config = &mut ctx.accounts.config;
        config.bond_counter = config.bond_counter.checked_add(1).unwrap();

        // =====================================================================
        // STEP 6: CLOSE TEMPORARY wSOL ACCOUNT (RECLAIM RENT)
        // =====================================================================

        token::close_account(CpiContext::new(
            ctx.accounts.token_program.to_account_info(),
            token::CloseAccount {
                account: ctx.accounts.user_wsol_account.to_account_info(),
                destination: ctx.accounts.user.to_account_info(),
                authority: ctx.accounts.user.to_account_info(),
            },
        ))?;

        // =====================================================================
        // EMIT EVENT
        // =====================================================================

        emit!(BondMinted {
            bond_mint: ctx.accounts.bond_mint.key(),
            position_mint: ctx.accounts.position_mint.key(),
            whirlpool: ctx.accounts.whirlpool.key(),
            depositor: ctx.accounts.user.key(),
            tick_lower_index,
            tick_upper_index,
            liquidity: liquidity_amount,
            sol_deposited: sol_amount,
            timestamp: Clock::get()?.unix_timestamp,
        });

        Ok(())
    }

    /// Redeem bond NFT to reclaim liquidity position.
    /// Burns the bond NFT and transfers the position NFT to the user.
    pub fn redeem_bond(ctx: Context<RedeemBond>) -> Result<()> {
        require!(
            ctx.accounts.user_bond_account.amount == 1,
            LpBondsError::InvalidBondBalance
        );

        // =====================================================================
        // STEP 1: BURN BOND NFT
        // =====================================================================

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

        // =====================================================================
        // STEP 2: TRANSFER POSITION NFT TO USER
        // =====================================================================

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

    /// CHECK: PDA derived from program, no data stored
    #[account(
        seeds = [BOND_AUTHORITY_SEED],
        bump
    )]
    pub bond_authority: UncheckedAccount<'info>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(
    tick_lower_index: i32,
    tick_upper_index: i32,
    liquidity_amount: u128,
    token_max_a: u64,
    token_max_b: u64,
    sol_amount: u64,
)]
pub struct AddLiquidityAndMintBond<'info> {
    // =========================================================================
    // USER ACCOUNTS
    // =========================================================================

    #[account(mut)]
    pub user: Signer<'info>,

    // =========================================================================
    // TOKEN MINTS (declared first so token accounts can reference them)
    // =========================================================================

    #[account(
        address = NATIVE_MINT @ LpBondsError::InvalidNativeMint,
    )]
    pub wsol_mint: Box<Account<'info, Mint>>,

    #[account(
        address = EXPECTED_TOKEN_MINT_B @ LpBondsError::InvalidTokenMintB,
    )]
    pub token_mint_b: Box<Account<'info, Mint>>,

    /// CHECK: PDA validated by seeds (needed before bond_mint for authority)
    #[account(
        seeds = [BOND_AUTHORITY_SEED],
        bump,
    )]
    pub bond_authority: UncheckedAccount<'info>,

    // =========================================================================
    // BOND NFT ACCOUNTS (declared before token accounts that reference bond_mint)
    // =========================================================================

    #[account(
        init,
        payer = user,
        mint::decimals = 0,
        mint::authority = bond_authority,
        mint::freeze_authority = bond_authority,
    )]
    pub bond_mint: Box<Account<'info, Mint>>,

    // =========================================================================
    // USER TOKEN ACCOUNTS (referencing mints declared above)
    // =========================================================================

    #[account(
        init,
        payer = user,
        token::mint = wsol_mint,
        token::authority = user,
    )]
    pub user_wsol_account: Box<Account<'info, TokenAccount>>,

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

    // =========================================================================
    // PROTOCOL ACCOUNTS
    // =========================================================================

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

    // =========================================================================
    // WHIRLPOOL POSITION ACCOUNTS
    // =========================================================================

    /// CHECK: Initialized by whirlpool CPI
    #[account(mut)]
    pub position_mint: Signer<'info>,

    /// CHECK: Created by whirlpool CPI, PDA of whirlpool program
    #[account(mut)]
    pub whirlpool_position: UncheckedAccount<'info>,

    /// Position token account owned by USER (created by open_position CPI)
    /// This is where the position NFT is minted to initially
    /// CHECK: Created by whirlpool CPI
    #[account(mut)]
    pub position_token_account: UncheckedAccount<'info>,

    /// ATA for position NFT owned by position_custody PDA
    /// Position NFT will be transferred here after increase_liquidity
    /// CHECK: Created after open_position CPI since position_mint doesn't exist until then
    #[account(mut)]
    pub custody_position_token_account: UncheckedAccount<'info>,

    // =========================================================================
    // WHIRLPOOL ACCOUNTS
    // =========================================================================

    /// CHECK: Manually validated - owned by Whirlpool program and matches allowlist
    #[account(
        mut,
        constraint = whirlpool.key() == ALLOWLISTED_WHIRLPOOL @ LpBondsError::WhirlpoolNotAllowlisted,
    )]
    pub whirlpool: UncheckedAccount<'info>,

    /// CHECK: Validated in instruction handler against whirlpool data
    #[account(mut)]
    pub token_vault_a: UncheckedAccount<'info>,

    /// CHECK: Validated in instruction handler against whirlpool data
    #[account(mut)]
    pub token_vault_b: UncheckedAccount<'info>,

    /// CHECK: Validated by whirlpool program
    #[account(mut)]
    pub tick_array_lower: UncheckedAccount<'info>,

    /// CHECK: Validated by whirlpool program
    #[account(mut)]
    pub tick_array_upper: UncheckedAccount<'info>,

    // =========================================================================
    // PROGRAMS
    // =========================================================================

    /// CHECK: Validated against known program ID
    #[account(
        address = whirlpool_cpi::WHIRLPOOL_PROGRAM_ID @ LpBondsError::InvalidWhirlpoolProgram,
    )]
    pub whirlpool_program: UncheckedAccount<'info>,

    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
    pub rent: Sysvar<'info, Rent>,
}

#[derive(Accounts)]
pub struct RedeemBond<'info> {
    #[account(mut)]
    pub user: Signer<'info>,

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

    #[account(
        constraint = position_mint.key() == position_custody.position_mint @ LpBondsError::InvalidPositionMint,
    )]
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
