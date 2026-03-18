use anchor_lang::prelude::*;

/// Emitted when an NFT is minted in public sale. Maps to EVM `Minted` event.
#[event]
pub struct Minted {
    pub collection: Pubkey,
    pub user: Pubkey,
    pub quantity: u64,
    pub token_index: u64,
}

/// Emitted when an NFT is minted in presale. Maps to EVM `PresaleMinted` event.
#[event]
pub struct PresaleMinted {
    pub collection: Pubkey,
    pub user: Pubkey,
    pub quantity: u64,
    pub token_index: u64,
}

/// Emitted when owner mints an NFT. Maps to EVM `OwnerMinted` event.
#[event]
pub struct OwnerMinted {
    pub collection: Pubkey,
    pub recipient: Pubkey,
    pub quantity: u64,
    pub token_index: u64,
}

/// Emitted when an NFT is refunded. Maps to EVM `Refund` event.
#[event]
pub struct Refunded {
    pub collection: Pubkey,
    pub user: Pubkey,
    pub mint: Pubkey,
    pub token_index: u64,
    pub refund_amount: u64,
}

/// Emitted when max user mint amount is changed.
#[event]
pub struct MaxUserMintAmountChanged {
    pub collection: Pubkey,
    pub new_max_user_mint_amount: u64,
}

/// Emitted when max tx mint amount is changed.
#[event]
pub struct MaxTxMintAmountChanged {
    pub collection: Pubkey,
    pub new_max_tx_mint_amount: u64,
}

/// Emitted when mint price is changed.
#[event]
pub struct MintPriceChanged {
    pub collection: Pubkey,
    pub new_mint_price: u64,
}

/// Emitted when presale max user mint amount is changed.
#[event]
pub struct PresaleMaxUserMintAmountChanged {
    pub collection: Pubkey,
    pub new_presale_max_user_mint_amount: u64,
}

/// Emitted when presale max tx mint amount is changed.
#[event]
pub struct PresaleMaxTxMintAmountChanged {
    pub collection: Pubkey,
    pub new_presale_max_tx_mint_amount: u64,
}

/// Emitted when presale mint price is changed.
#[event]
pub struct PresaleMintPriceChanged {
    pub collection: Pubkey,
    pub new_presale_mint_price: u64,
}

/// Emitted when a whitelist entry is added.
#[event]
pub struct WhitelistAdded {
    pub collection: Pubkey,
    pub user: Pubkey,
    pub mint_limit: u64,
}

/// Emitted when a whitelist entry is removed.
#[event]
pub struct WhitelistRemoved {
    pub collection: Pubkey,
    pub user: Pubkey,
}

/// Emitted when presale status is toggled. Note: emits NEW state (fixes EVM bug IB15).
#[event]
pub struct PresaleToggled {
    pub collection: Pubkey,
    pub presale_active: bool,
}

/// Emitted when publicsale status is toggled. Note: emits NEW state (fixes EVM bug IB15).
#[event]
pub struct PublicsaleToggled {
    pub collection: Pubkey,
    pub publicsale_active: bool,
}

/// Emitted when base URI is set.
#[event]
pub struct BaseUriSet {
    pub collection: Pubkey,
    pub uri: String,
}

/// Emitted when collection is initialized (maps to EVM `CollectionLaunched`).
#[event]
pub struct CollectionInitialized {
    pub collection: Pubkey,
    pub authority: Pubkey,
    pub name: String,
    pub symbol: String,
    pub collection_type: u8,
}

/// Emitted when pause status is toggled.
#[event]
pub struct PauseToggled {
    pub collection: Pubkey,
    pub paused: bool,
}
