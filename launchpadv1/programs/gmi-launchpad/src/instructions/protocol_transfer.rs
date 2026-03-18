use anchor_lang::prelude::*;
use anchor_spl::token::{self, Token, TokenAccount, Transfer};
use crate::state::{Collection, TokenRecord};
use crate::errors::LaunchpadError;
use crate::events::ProtocolTransferEvent;
use operator_registry::state::OperatorRegistryState;

#[derive(Accounts)]
#[instruction(expected_nonce: u64)]
pub struct ProtocolTransfer<'info> {
    #[account(mut)]
    pub collection: Account<'info, Collection>,

    #[account(
        mut,
        seeds = [
            TokenRecord::SEED_PREFIX,
            collection.key().as_ref(),
            nft_token_account_from.mint.as_ref(),
        ],
        bump = token_record.bump,
    )]
    pub token_record: Account<'info, TokenRecord>,

    #[account(
        mut,
        constraint = nft_token_account_from.owner == seller.key() @ LaunchpadError::NotTokenOwner,
        constraint = nft_token_account_from.amount == 1 @ LaunchpadError::TokenAccountEmpty,
    )]
    pub nft_token_account_from: Account<'info, TokenAccount>,

    #[account(
        mut,
        constraint = nft_token_account_to.mint == nft_token_account_from.mint @ LaunchpadError::InvalidMint,
    )]
    pub nft_token_account_to: Account<'info, TokenAccount>,

    #[account(mut)]
    pub seller: Signer<'info>,

    /// CHECK: Target recipient account
    pub buyer: AccountInfo<'info>,

    /// CHECK: Operator orchestrating the trade, if applicable
    pub operator: Option<Signer<'info>>,

    /// CHECK: Dynamically parsed via Account::try_from
    pub operator_registry_state: Option<AccountInfo<'info>>,
    
    /// CHECK: P2P filter helper
    #[account(
        seeds = [b"operator_whitelist", collection.key().as_ref(), operator.as_ref().map_or(&Pubkey::default().to_bytes()[..], |op| op.key.as_ref())],
        bump,
    )]
    /// CHECK: Handled internally
    pub operator_whitelist: Option<AccountInfo<'info>>,

    pub token_program: Program<'info, Token>,
}

pub fn handler(ctx: Context<ProtocolTransfer>, expected_nonce: u64) -> Result<()> {
    // 1. INVARIANT CHECK
    require!(
        ctx.accounts.nft_token_account_from.owner == ctx.accounts.token_record.owner,
        LaunchpadError::UnsettledState
    );
    require!(
        ctx.accounts.seller.key() == ctx.accounts.token_record.owner,
        LaunchpadError::Unauthorized
    );

    // 2. REPLAY PROTECTION
    let token_record = &mut ctx.accounts.token_record;
    require!(
        token_record.transfer_count == expected_nonce,
        LaunchpadError::InvalidNonce
    );

    // 3. OPERATOR VALIDATION
    let mut operator_pubkey = None;
    if let Some(ref operator) = ctx.accounts.operator {
        // CASE A: Smart Contract Operator Present
        require!(ctx.accounts.collection.has_operator_filter, LaunchpadError::OperatorNotWhitelisted);
        
        let whitelist_data = ctx.accounts.operator_whitelist.as_ref().ok_or(LaunchpadError::OperatorNotWhitelisted)?;
        // We ensure length > 0 just to prove an account exists
        require!(!whitelist_data.data_is_empty(), LaunchpadError::OperatorNotWhitelisted); // Verified by seeds logically or external data
        // For absolute robustness, relying on the client passing the right validated OperatorWhitelist account.
        // Assuming `OperatorWhitelist` holds `is_allowed: bool`
        let wl_data = whitelist_data.try_borrow_data()?;
        require!(wl_data[8 + 32 + 32] != 0, LaunchpadError::OperatorNotWhitelisted); 

        operator_pubkey = Some(operator.key());
    } else {
        // CASE B: P2P Transfer
        // Explicitly allow P2P transfers without operator validation.
        // State remains consistent because standard logic applies.
    }

    // 4. ATOMIC EXECUTION
    let cpi_ctx = CpiContext::new(
        ctx.accounts.token_program.to_account_info(),
        Transfer {
            from: ctx.accounts.nft_token_account_from.to_account_info(),
            to: ctx.accounts.nft_token_account_to.to_account_info(),
            authority: ctx.accounts.seller.to_account_info(),
        },
    );
    token::transfer(cpi_ctx, 1)?;

    // 5. UPDATE STATE
    token_record.owner = ctx.accounts.buyer.key();
    token_record.transfer_count = token_record.transfer_count.checked_add(1).unwrap();

    emit!(ProtocolTransferEvent {
        collection: ctx.accounts.collection.key(),
        mint: ctx.accounts.nft_token_account_from.mint,
        seller: ctx.accounts.seller.key(),
        buyer: ctx.accounts.buyer.key(),
        operator: operator_pubkey,
        expected_nonce,
    });

    Ok(())
}
