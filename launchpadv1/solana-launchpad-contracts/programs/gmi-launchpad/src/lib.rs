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

    /// Initialize a new NFT collection with all parameters.
    pub fn initialize_collection(
        ctx: Context<InitializeCollection>,
        params: initialize_collection::InitializeCollectionParams,
    ) -> Result<()> {
        initialize_collection::handler(ctx, params)
    }

    /// Public sale mint.
    pub fn mint_public(
        ctx: Context<MintPublic>,
        quantity: u64,
    ) -> Result<()> {
        mint_public::handler(ctx, quantity)
    }

    /// Presale mint (whitelist-gated).
    pub fn mint_presale(
        ctx: Context<MintPresale>,
        quantity: u64,
    ) -> Result<()> {
        mint_presale::handler(ctx, quantity)
    }

    /// Owner/authority free mint.
    pub fn mint_owner(
        ctx: Context<MintOwner>,
        quantity: u64,
    ) -> Result<()> {
        mint_owner::handler(ctx, quantity)
    }

    /// Refund NFT — burn token, return SOL from vault.
    pub fn refund_nft(ctx: Context<RefundNft>) -> Result<()> {
        refund_nft::handler(ctx)
    }

    /// Configure public sale parameters.
    pub fn configure_publicsale(
        ctx: Context<ConfigurePublicsale>,
        mint_price: Option<u64>,
        max_user_mint_amount: u64,
        max_tx_mint_amount: u64,
        publicsale_active: bool,
    ) -> Result<()> {
        configure_publicsale::handler(ctx, mint_price, max_user_mint_amount, max_tx_mint_amount, publicsale_active)
    }

    /// Configure presale parameters.
    pub fn configure_presale(
        ctx: Context<ConfigurePresale>,
        presale_mint_price: Option<u64>,
        presale_max_user_mint_amount: u64,
        presale_max_tx_mint_amount: u64,
        presale_active: bool,
    ) -> Result<()> {
        configure_presale::handler(ctx, presale_mint_price, presale_max_user_mint_amount, presale_max_tx_mint_amount, presale_active)
    }

    /// Toggle presale status.
    pub fn toggle_presale(ctx: Context<TogglePresale>) -> Result<()> {
        toggle_presale::handler(ctx)
    }

    /// Toggle publicsale status.
    pub fn toggle_publicsale(ctx: Context<TogglePublicsale>) -> Result<()> {
        toggle_publicsale::handler(ctx)
    }

    /// Toggle pause.
    pub fn toggle_pause(ctx: Context<TogglePause>) -> Result<()> {
        toggle_pause::handler(ctx)
    }

    /// Set base URI.
    pub fn set_base_uri(ctx: Context<SetBaseUri>, uri: String) -> Result<()> {
        set_base_uri::handler(ctx, uri)
    }

    /// Add whitelist entry.
    pub fn add_whitelist(
        ctx: Context<AddWhitelist>,
        mint_limit: u64,
    ) -> Result<()> {
        add_whitelist::handler(ctx, mint_limit)
    }

    /// Remove whitelist entry.
    pub fn remove_whitelist(ctx: Context<RemoveWhitelist>) -> Result<()> {
        remove_whitelist::handler(ctx)
    }
}
