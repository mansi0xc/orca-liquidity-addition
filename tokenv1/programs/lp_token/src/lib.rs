// Copyright 2025 Energi Core
//
// Solana migration of EVM LPToken.sol
// Original: contracts/lp-token/LPToken.sol
//
// Uses the SPL Token Program for all balance accounting.
// Wraps SPL calls with the same access control semantics as the EVM contract.

use anchor_lang::prelude::*;

pub mod errors;
pub mod events;
pub mod instructions;
pub mod state;

use errors::*;
use instructions::*;
use state::*;

// Placeholder ID — replace with the output of `anchor keys gen` before deployment.
// Run: solana-keygen new -o target/deploy/lp_token-keypair.json
//      and copy the printed pubkey into declare_id! below.
declare_id!("Fg6PaFpoGXkYsidMpWTK6W2BeZ7FEfcYkg476zPFsLnS");

/// PDA seed for the TokenState account.
/// Seeds: [TOKEN_STATE_SEED, mint_pubkey]
pub const TOKEN_STATE_SEED: &[u8] = b"token_state";

/// PDA seed for MinterRecord accounts.
/// Seeds: [MINTER_SEED, token_state_pubkey, minter_pubkey]
pub const MINTER_SEED: &[u8] = b"minter";

#[program]
pub mod lp_token {
    use super::*;

    // ============================== Initialization ==============================

    /// Initialize a new LP token mint.
    ///
    /// EVM: LPToken.initialize(name_, symbol_, owner_, chainId_)
    ///
    /// Creates the SPL mint and the TokenState PDA that governs it.
    /// The token_state PDA becomes the mint_authority.
    pub fn initialize_mint(
        mut ctx: Context<InitializeMint>,
        params: InitializeMintParams,
    ) -> Result<()> {
        InitializeMint::apply(&mut ctx, &params)
    }

    // ============================== Mint / Burn ==============================

    /// Mint new LP tokens to a recipient.
    ///
    /// EVM: LPToken.mint(address _account, uint256 _amount)
    ///   - Caller must be owner or a registered minter
    ///   - Reverts when paused
    ///   - No max supply cap
    pub fn mint_tokens(ctx: Context<MintTokens>, amount: u64) -> Result<()> {
        MintTokens::apply(&mut { ctx }, amount)
    }

    /// Burn LP tokens from a token account.
    ///
    /// EVM: LPToken.burn(address _account, uint256 _amount)
    ///   - Caller must be owner or a registered minter
    ///   - Token account owner must also sign (Solana security improvement)
    ///   - Reverts when paused
    pub fn burn_tokens(ctx: Context<BurnTokens>, amount: u64) -> Result<()> {
        BurnTokens::apply(&mut { ctx }, amount)
    }

    // ============================== Governance ==============================

    /// Register or deregister a minter address.
    ///
    /// EVM: LPToken.updateMinter(address _account, bool _isMinter)
    ///   - Only owner may call
    ///   - Reverts on duplicate operation
    pub fn update_minter(
        mut ctx: Context<UpdateMinter>,
        params: UpdateMinterParams,
    ) -> Result<()> {
        UpdateMinter::apply(&mut ctx, &params)
    }

    /// Pause or unpause mint/burn operations.
    ///
    /// EVM: LPToken.pause() / LPToken.unpause()
    ///   - Only owner may call
    ///   - pause() requires not-paused; unpause() requires paused
    ///   - Regular SPL transfers are NOT affected (matches LPToken behavior)
    pub fn set_pause(ctx: Context<SetPause>, paused: bool) -> Result<()> {
        SetPause::apply(&mut { ctx }, paused)
    }

    // ============================== Standard ERC20 Equivalents ==============================

    /// Transfer LP tokens between accounts.
    ///
    /// EVM: ERC20.transfer / ERC20.transferFrom
    ///   - No custom guards (LPToken does not override _transfer)
    ///   - Not blocked by pause (matches LPToken behavior)
    ///   - For delegated transfers, caller should be the SPL delegate
    ///
    /// Note: Users may also call the SPL Token Program directly.
    pub fn transfer_tokens(ctx: Context<TransferTokens>, amount: u64) -> Result<()> {
        TransferTokens::apply(&mut { ctx }, amount)
    }

    /// Approve a delegate allowance on a token account.
    ///
    /// EVM: ERC20.approve(address spender, uint256 amount)
    ///   - No custom guards (LPToken does not override _approve)
    ///   - Not blocked by pause (matches LPToken behavior)
    ///
    /// Note: Users may also call the SPL Token Program directly.
    pub fn approve_delegate(ctx: Context<ApproveDelegate>, amount: u64) -> Result<()> {
        ApproveDelegate::apply(&mut { ctx }, amount)
    }
}
