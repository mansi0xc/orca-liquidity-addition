use anchor_lang::prelude::*;

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq, Debug)]
pub enum CollectionType {
    Standard,
    Refundable100,
    Refundable80,
}

#[account]
pub struct Collection {
    pub authority: Pubkey,
    pub collection_id: [u8; 32],
    pub collection_type: CollectionType,
    pub has_operator_filter: bool,
    pub operator_registry: Pubkey,

    // Supply
    pub max_mint_supply: u64,
    pub minted_amount: u64,
    pub total_mints: u64,
    pub refund_counter: u64,

    // Pricing
    pub mint_price: u64,
    pub presale_mint_price: u64,

    // Limits
    pub max_user_mint_amount: u64,
    pub max_tx_mint_amount: u64,
    pub presale_max_user_mint_amount: u64,
    pub presale_max_tx_mint_amount: u64,

    // Sale Status
    pub presale_active: bool,
    pub publicsale_active: bool,
    pub paused: bool,

    // Reserved (for Refundable variants)
    pub reserved_nfts: u64,
    pub reserved_mints: u64,

    // Metadata
    pub name: String,
    pub symbol: String,
    pub base_uri: String,

    // R80 Remint Identity pool size
    pub available_remints: u64,
    
    // Global Bot Mitigation Cooldown
    pub global_last_mint_slot: u64,
    pub min_slot_cooldown: u64,         // User cooldown (e.g., 5 slots)
    pub global_min_slot_cooldown: u64,  // Global cooldown (e.g., 1 slot)

    pub bump: u8,
    pub vault_bump: u8,
}

impl Collection {
    pub const SEED_PREFIX: &'static [u8] = b"collection";
    pub const VAULT_SEED_PREFIX: &'static [u8] = b"vault";

    pub fn space(name_len: usize, symbol_len: usize, base_uri_len: usize) -> usize {
        8  // discriminator
        + 32 // authority
        + 32 // collection_id
        + 1  // collection_type
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
        + 4 + name_len 
        + 4 + symbol_len 
        + 4 + base_uri_len
        + 8  // available_remints
        + 8  // global_last_mint_slot
        + 8  // min_slot_cooldown
        + 8  // global_min_slot_cooldown
        + 1  // bump
        + 1  // vault_bump
    }
}

#[account]
pub struct MintCounter {
    pub collection: Pubkey,
    pub user: Pubkey,
    pub number_minted: u64,
    pub presale_number_minted: u64,
    pub last_mint_slot: u64,
    pub bump: u8,
}

impl MintCounter {
    pub const SEED_PREFIX: &'static [u8] = b"mint_counter";
    pub const SIZE: usize = 8 + 32 + 32 + 8 + 8 + 8 + 1;
}

#[account]
pub struct WhitelistEntry {
    pub collection: Pubkey,
    pub user: Pubkey,
    pub mint_limit: u64,
    pub bump: u8,
}

impl WhitelistEntry {
    pub const SEED_PREFIX: &'static [u8] = b"whitelist";
    pub const SIZE: usize = 8 + 32 + 32 + 8 + 1;
}

#[account]
pub struct TokenRecord {
    pub collection: Pubkey,
    pub mint: Pubkey,
    pub token_index: u64,
    pub refund_price: u64,
    pub is_owner_mint: bool,
    
    // Explicit Ownership & Settlement Enforcement
    pub owner: Pubkey,
    
    // Replay Protection
    pub transfer_count: u64,
    
    // Fee Sync Mechanics
    pub original_mint_price: u64,
    pub protocol_fee_bps: u64,
    
    pub bump: u8,
}

impl TokenRecord {
    pub const SEED_PREFIX: &'static [u8] = b"token_record";
    pub const SIZE: usize = 8 + 32 + 32 + 8 + 8 + 1 + 32 + 8 + 8 + 8 + 1;
}

#[account]
pub struct RefundBitmap {
    pub collection: Pubkey,
    pub search_cursor: u16,
    pub bitmap: [u8; 1250], // 10,000 bits for 10k supply max
    pub bump: u8,
}

impl RefundBitmap {
    pub const SEED_PREFIX: &'static [u8] = b"refund_bitmap";
    // 8 + 32 + 2 + 1250 + 1
    pub const SIZE: usize = 1293;
}
