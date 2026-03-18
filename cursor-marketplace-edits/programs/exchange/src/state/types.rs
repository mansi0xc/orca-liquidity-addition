use anchor_lang::prelude::*;

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug, PartialEq, Eq)]
pub enum AssetClass {
    Sol,
    WrappedSol,
    SplToken,
    Nft,
    SemiFungible,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug)]
pub struct AssetType {
    pub asset_class: AssetClass,
    pub mint: Pubkey,
    pub token_id: u64,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug)]
pub struct Asset {
    pub asset_type: AssetType,
    pub value: u64,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug)]
pub struct Order {
    pub maker: Pubkey,
    pub make_asset: Asset,
    pub taker: Pubkey,
    pub take_asset: Asset,
    pub salt: u64,
    pub start: i64,
    pub end: i64,
    pub data_type: [u8; 4],
    pub data: Vec<u8>,
    pub collection_bid: bool,
}

/// V1 data type identifier: keccak256("V1") truncated to 4 bytes.
/// On Solana we use a fixed constant.
pub const DATA_TYPE_V1: [u8; 4] = [0xa0, 0x83, 0x2e, 0xf7];
/// Empty order data sentinel (0xffffffff)
pub const DATA_TYPE_EMPTY: [u8; 4] = [0xff, 0xff, 0xff, 0xff];

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug)]
pub struct DataV1 {
    pub payouts: Vec<Part>,
    pub origin_fees: Vec<Part>,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug)]
pub struct Part {
    pub account: Pubkey,
    pub value: u16,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug, PartialEq, Eq)]
pub enum FeeSide {
    Make,
    Take,
    None,
}

#[derive(Clone, Debug)]
pub struct FillResult {
    pub right_order_take_value: u64,
    pub left_order_take_value: u64,
}

/// Transfer direction constants
pub const TRANSFER_TO_MAKER: u8 = 0;
pub const TRANSFER_TO_TAKER: u8 = 1;

/// Transfer type constants
pub const TRANSFER_TYPE_PROTOCOL: u8 = 0;
pub const TRANSFER_TYPE_ROYALTY: u8 = 1;
pub const TRANSFER_TYPE_ORIGIN: u8 = 2;
pub const TRANSFER_TYPE_PAYOUT: u8 = 3;
