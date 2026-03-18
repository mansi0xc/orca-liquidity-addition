use anchor_lang::prelude::*;

/// Collection type — determines refund behavior and revenue split.
/// Maps to the 3 EVM contract tiers (× 2 OperatorFilter variants handled by `has_operator_filter`).
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq, Debug)]
pub enum CollectionType {
    /// Standard: No refund. 100% of mint price goes to owner immediately.
    /// Maps to: GMIERC721 / GMIERC721C
    Standard,
    /// Refundable100: Full refund at mint price. Funds held in vault.
    /// Maps to: GMIERC721R / GMIERC721RC
    Refundable100,
    /// Refundable80: 80% refund. 20% goes to owner on mint, 80% held in vault.
    /// Maps to: GMIERC721R80 / GMIERC721R80C
    Refundable80,
}

/// Main collection state account.
/// Seeds: ["collection", collection_id]
#[account]
pub struct Collection {
    /// Authority (owner) of the collection
    pub authority: Pubkey,

    /// Unique collection identifier (used in PDA seeds)
    pub collection_id: [u8; 32],

    /// Collection type determines refund behavior
    pub collection_type: CollectionType,

    /// Whether operator filter (Creator Economy) is enabled
    pub has_operator_filter: bool,

    /// Operator registry pubkey (only used if has_operator_filter is true)
    pub operator_registry: Pubkey,

    // === Supply ===
    /// Maximum mintable supply
    pub max_mint_supply: u64,
    /// Current live minted count (decremented on refund). Invariant: <= max_mint_supply
    pub minted_amount: u64,
    /// Total ever minted (monotonically increasing, never decremented)
    pub total_mints: u64,
    /// Count of refunded NFTs
    pub refund_counter: u64,

    // === Pricing ===
    /// Public sale mint price (in lamports)
    pub mint_price: u64,
    /// Presale mint price (in lamports)
    pub presale_mint_price: u64,

    // === Limits ===
    /// Max mintable per user in public sale
    pub max_user_mint_amount: u64,
    /// Max mintable per transaction in public sale
    pub max_tx_mint_amount: u64,
    /// Max mintable per user in presale
    pub presale_max_user_mint_amount: u64,
    /// Max mintable per transaction in presale
    pub presale_max_tx_mint_amount: u64,

    // === Sale Status ===
    /// Whether presale is active
    pub presale_active: bool,
    /// Whether public sale is active
    pub publicsale_active: bool,
    /// Whether the collection is paused
    pub paused: bool,

    // === Reserved (for Refundable variants) ===
    /// Reserved NFTs for owner/free presale mints (20% of max supply for R variants)
    pub reserved_nfts: u64,
    /// Count of reserved mints used
    pub reserved_mints: u64,

    // === Metadata ===
    /// Collection name
    pub name: String,
    /// Collection symbol
    pub symbol: String,
    /// Base URI for metadata
    pub base_uri: String,

    // === PDA Seeds ===
    pub bump: u8,
    pub vault_bump: u8,
}

impl Collection {
    pub const SEED_PREFIX: &'static [u8] = b"collection";
    pub const VAULT_SEED_PREFIX: &'static [u8] = b"vault";

    /// Calculate account space. Strings are variable, so we allow generous defaults.
    pub fn space(name_len: usize, symbol_len: usize, base_uri_len: usize) -> usize {
        8 // discriminator
        + 32 // authority
        + 32 // collection_id
        + 1  // collection_type enum
        + 1  // has_operator_filter
        + 32 // operator_registry
        + 8  // max_mint_supply
        + 8  // minted_amount
        + 8  // total_mints
        + 8  // refund_counter
        + 8  // mint_price
        + 8  // presale_mint_price
        + 8  // max_user_mint_amount
        + 8  // max_tx_mint_amount
        + 8  // presale_max_user_mint_amount
        + 8  // presale_max_tx_mint_amount
        + 1  // presale_active
        + 1  // publicsale_active
        + 1  // paused
        + 8  // reserved_nfts
        + 8  // reserved_mints
        + 4 + name_len       // name (string length prefix + data)
        + 4 + symbol_len     // symbol
        + 4 + base_uri_len   // base_uri
        + 1  // bump
        + 1  // vault_bump
    }
}

/// Per-user mint counter for a specific collection.
/// Seeds: ["mint_counter", collection, user]
#[account]
pub struct MintCounter {
    /// The collection this counter belongs to
    pub collection: Pubkey,
    /// The user this counter tracks
    pub user: Pubkey,
    /// Number of NFTs minted in public sale
    pub number_minted: u64,
    /// Number of NFTs minted in presale
    pub presale_number_minted: u64,
    /// PDA bump
    pub bump: u8,
}

impl MintCounter {
    pub const SEED_PREFIX: &'static [u8] = b"mint_counter";
    pub const SIZE: usize = 8 + 32 + 32 + 8 + 8 + 1;
}

/// Whitelist entry for a user in a specific collection.
/// Seeds: ["whitelist", collection, user]
#[account]
pub struct WhitelistEntry {
    /// The collection this whitelist applies to
    pub collection: Pubkey,
    /// The whitelisted user
    pub user: Pubkey,
    /// Maximum number of presale mints allowed (NOT a boolean — maps to EVM `whitelists[user]`)
    pub mint_limit: u64,
    /// PDA bump
    pub bump: u8,
}

impl WhitelistEntry {
    pub const SEED_PREFIX: &'static [u8] = b"whitelist";
    pub const SIZE: usize = 8 + 32 + 32 + 8 + 1;
}

/// Per-NFT record tracking refund price and owner-mint status.
/// Seeds: ["token_record", collection, mint]
#[account]
pub struct TokenRecord {
    /// The collection this token belongs to
    pub collection: Pubkey,
    /// The SPL token mint address
    pub mint: Pubkey,
    /// Sequential token index within the collection
    pub token_index: u64,
    /// Refund price in lamports (set at mint time, immutable)
    pub refund_price: u64,
    /// Whether this was minted by the owner (non-refundable)
    pub is_owner_mint: bool,
    /// PDA bump
    pub bump: u8,
}

impl TokenRecord {
    pub const SEED_PREFIX: &'static [u8] = b"token_record";
    pub const SIZE: usize = 8 + 32 + 32 + 8 + 8 + 1 + 1;
}
