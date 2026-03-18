use anchor_lang::prelude::*;

pub mod state;
pub mod errors;
pub mod events;
pub mod utils;
pub mod instructions;

use instructions::*;

declare_id!("Hd6oq17jeqbEpBKvDXy3UvBVrHhLztV9GZGfXkVEzQxU");

#[program]
pub mod gmi_launchpad {
    use super::*;

    pub fn initialize_collection(
        ctx: Context<InitializeCollection>,
        params: InitializeCollectionParams,
    ) -> Result<()> {
        initialize_collection::handler(ctx, params)
    }

    pub fn mint_public(ctx: Context<MintPublic>, _quantity: u64) -> Result<()> {
        // _quantity is deprecated locally per slot/cooldown redesign but kept for IDL/TS test compatibility.
        mint_public::handler(ctx)
    }

    pub fn mint_presale(ctx: Context<MintPresale>, _quantity: u64) -> Result<()> {
        // _quantity is deprecated locally per slot/cooldown redesign but kept for IDL/TS test compatibility.
        mint_presale::handler(ctx)
    }

    pub fn mint_owner(ctx: Context<MintOwner>, _quantity: u64) -> Result<()> {
        // _quantity is deprecated locally per slot/cooldown redesign but kept for IDL/TS test compatibility.
        mint_owner::handler(ctx)
    }

    pub fn refund_nft(ctx: Context<RefundNft>) -> Result<()> {
        refund_nft::handler(ctx)
    }

    pub fn protocol_transfer(ctx: Context<ProtocolTransfer>, expected_nonce: u64) -> Result<()> {
        protocol_transfer::handler(ctx, expected_nonce)
    }

    pub fn sync_ownership(ctx: Context<SyncOwnership>) -> Result<()> {
        sync_ownership::handler(ctx)
    }

    pub fn withdraw_vault(ctx: Context<WithdrawVault>) -> Result<()> {
        withdraw_vault::handler(ctx)
    }

    pub fn configure_publicsale(
        ctx: Context<ConfigurePublicsale>,
        mint_price: Option<u64>,
        max_user_mint_amount: u64,
        max_tx_mint_amount: u64,
        publicsale_active: bool,
    ) -> Result<()> {
        configure_publicsale::handler(ctx, mint_price, max_user_mint_amount, max_tx_mint_amount, publicsale_active)
    }

    pub fn configure_presale(
        ctx: Context<ConfigurePresale>,
        presale_mint_price: Option<u64>,
        presale_max_user_mint_amount: u64,
        presale_max_tx_mint_amount: u64,
        presale_active: bool,
    ) -> Result<()> {
        configure_presale::handler(ctx, presale_mint_price, presale_max_user_mint_amount, presale_max_tx_mint_amount, presale_active)
    }

    pub fn toggle_presale(ctx: Context<TogglePresale>) -> Result<()> {
        toggle_presale::handler(ctx)
    }

    pub fn toggle_publicsale(ctx: Context<TogglePublicsale>) -> Result<()> {
        toggle_publicsale::handler(ctx)
    }

    pub fn toggle_pause(ctx: Context<TogglePause>) -> Result<()> {
        toggle_pause::handler(ctx)
    }

    pub fn set_base_uri(ctx: Context<SetBaseUri>, uri: String) -> Result<()> {
        set_base_uri::handler(ctx, uri)
    }

    pub fn add_whitelist(ctx: Context<AddWhitelist>, mint_limit: u64) -> Result<()> {
        add_whitelist::handler(ctx, mint_limit)
    }

    pub fn remove_whitelist(ctx: Context<RemoveWhitelist>) -> Result<()> {
        remove_whitelist::handler(ctx)
    }
}
