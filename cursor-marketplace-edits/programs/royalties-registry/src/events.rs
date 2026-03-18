use anchor_lang::prelude::*;

#[event]
pub struct RoyaltiesSetForToken {
    pub collection_mint: Pubkey,
    pub token_id: u64,
    pub recipients: Vec<Pubkey>,
    pub bps_values: Vec<u16>,
    pub setter_type: u8, // 0 = OWNER, 1 = CREATOR
}

#[event]
pub struct RoyaltiesSetForCollection {
    pub collection_mint: Pubkey,
    pub recipients: Vec<Pubkey>,
    pub bps_values: Vec<u16>,
}
