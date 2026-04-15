use crate::*;
use anchor_spl::token::{Mint, Token};

/// Initialize a new LP token mint with its governance state.
///
/// EVM equivalent: LPToken.initialize(name_, symbol_, owner_, chainId_)
///
/// Creates:
///   - A fresh SPL mint with `token_state` PDA as mint_authority and freeze_authority
///   - The `TokenState` PDA that holds governance state
///
/// Note on decimals:
///   EVM LPToken uses 18 decimals. SPL token balances are u64; with 18 decimals
///   the maximum representable supply is only ~18 tokens. The standard Solana
///   practice is 9 decimals, allowing ~18.4 billion tokens at full precision.
///   The caller should pass `decimals = 9` unless there is a specific reason.
///
/// Note on name/symbol:
///   The base SPL mint does not store name or symbol. Use the Metaplex Token
///   Metadata program post-initialization to attach human-readable metadata.
///   The evm_chain_id field records which EVM chain this token corresponds to.
#[derive(Accounts)]
#[instruction(params: InitializeMintParams)]
pub struct InitializeMint<'info> {
    /// Pays for account creation
    #[account(mut)]
    pub payer: Signer<'info>,

    /// Fresh keypair for the SPL mint account.
    /// The client must generate a new keypair and pass it as a signer.
    #[account(
        init,
        payer = payer,
        mint::decimals = params.decimals,
        mint::authority = token_state,
        mint::freeze_authority = token_state,
    )]
    pub token_mint: Account<'info, Mint>,

    /// Program governance PDA.
    /// Bound to this specific mint via seeds.
    #[account(
        init,
        payer = payer,
        space = 8 + TokenState::INIT_SPACE,
        seeds = [TOKEN_STATE_SEED, token_mint.key().as_ref()],
        bump,
    )]
    pub token_state: Account<'info, TokenState>,

    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
    pub rent: Sysvar<'info, Rent>,
}

impl InitializeMint<'_> {
    pub fn apply(ctx: &mut Context<InitializeMint>, params: &InitializeMintParams) -> Result<()> {
        // Defense-in-depth: prevent initializing with a zero-address owner.
        // The EVM initialize() does NOT enforce this (it calls _transferOwnership
        // which has no zero check), but initializing with Pubkey::default() would
        // create a permanently ungovernable token.
        require!(
            params.owner != Pubkey::default(),
            LPTokenError::InvalidOwner
        );

        let token_state = &mut ctx.accounts.token_state;
        token_state.owner = params.owner;
        token_state.pending_owner = Pubkey::default();
        token_state.is_paused = false;
        token_state.evm_chain_id = params.evm_chain_id;
        token_state.bump = ctx.bumps.token_state;

        emit!(events::MintInitialized {
            mint: ctx.accounts.token_mint.key(),
            owner: params.owner,
            evm_chain_id: params.evm_chain_id,
            decimals: params.decimals,
        });

        Ok(())
    }
}

#[derive(Clone, AnchorSerialize, AnchorDeserialize)]
pub struct InitializeMintParams {
    /// Initial admin address. Maps to EVM owner_.
    pub owner: Pubkey,
    /// EVM chain ID this LP token is associated with. Maps to EVM chainId_.
    pub evm_chain_id: u64,
    /// Token decimals. Use 9 for Solana standard.
    pub decimals: u8,
}
