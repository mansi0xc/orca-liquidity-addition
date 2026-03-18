use anchor_lang::prelude::*;

#[event]
pub struct CollectionInitialized {
    pub collection: Pubkey,
    pub authority: Pubkey,
    pub name: String,
    pub symbol: String,
    pub collection_type: u8,
}

#[event]
pub struct Minted {
    pub collection: Pubkey,
    pub user: Pubkey,
    pub token_index: u64,
    pub is_remint: bool,
}

#[event]
pub struct PresaleMinted {
    pub collection: Pubkey,
    pub user: Pubkey,
    pub token_index: u64,
    pub is_remint: bool,
}

#[event]
pub struct Refunded {
    pub collection: Pubkey,
    pub user: Pubkey,
    pub mint: Pubkey,
    pub token_index: u64,
    pub refund_amount: u64,
}

#[event]
pub struct ProtocolTransferEvent {
    pub collection: Pubkey,
    pub mint: Pubkey,
    pub seller: Pubkey,
    pub buyer: Pubkey,
    pub operator: Option<Pubkey>,
    pub expected_nonce: u64,
}

#[event]
pub struct OwnershipSynced {
    pub collection: Pubkey,
    pub mint: Pubkey,
    pub new_owner: Pubkey,
    pub fee_paid: u64,
}

#[event]
pub struct PresaleToggled {
    pub collection: Pubkey,
    pub presale_active: bool,
}

#[event]
pub struct PublicsaleToggled {
    pub collection: Pubkey,
    pub publicsale_active: bool,
}
