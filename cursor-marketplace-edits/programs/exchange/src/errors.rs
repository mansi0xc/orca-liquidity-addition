use anchor_lang::prelude::*;

#[error_code]
pub enum ExchangeError {
    #[msg("Exchange is paused")]
    Paused,
    #[msg("Maker cannot pay with native SOL")]
    MakerCannotPayWithSol,
    #[msg("Token not allowed for trading")]
    TokenNotAllowed,
    #[msg("Order has expired")]
    OrderExpired,
    #[msg("Order has not started yet")]
    OrderNotStarted,
    #[msg("Asset classes are incompatible")]
    AssetClassMismatch,
    #[msg("Invalid signature")]
    InvalidSignature,
    #[msg("Match allowance has expired")]
    MatchAllowanceExpired,
    #[msg("Invalid order book signature")]
    InvalidOrderBookSignature,
    #[msg("Order has been cancelled")]
    OrderCancelled,
    #[msg("Nothing to fill")]
    NothingToFill,
    #[msg("Royalties exceed 50% cap")]
    RoyaltiesTooHigh,
    #[msg("Payout sum does not equal 10000 bps")]
    InvalidPayoutSum,
    #[msg("Not the order maker")]
    NotOrderMaker,
    #[msg("Zero salt orders cannot be cancelled")]
    ZeroSaltCannotCancel,
    #[msg("Counterparty mismatch")]
    CounterpartyMismatch,
    #[msg("Assets do not match")]
    AssetsDoNotMatch,
    #[msg("Fill overflow")]
    FillOverflow,
    #[msg("Rounding error exceeds threshold")]
    RoundingError,
    #[msg("Division by zero")]
    DivisionByZero,
    #[msg("Unauthorized")]
    Unauthorized,
    #[msg("Invalid protocol fee")]
    InvalidProtocolFee,
    #[msg("Collection bid must use wSOL or SPL token")]
    InvalidCollectionBidAsset,
    #[msg("Invalid collection bid taker order")]
    InvalidCollectionBidTaker,
    #[msg("Fill unable to complete")]
    FillUnableToComplete,
    #[msg("Cannot transfer to zero address")]
    ZeroAddressTransfer,
    #[msg("Transfer amount cannot be zero")]
    ZeroAmountTransfer,
    #[msg("Unknown asset class")]
    UnknownAssetClass,
    #[msg("Invalid order data type")]
    InvalidOrderDataType,
    #[msg("Arithmetic overflow")]
    ArithmeticOverflow,
    #[msg("Invalid remaining accounts")]
    InvalidRemainingAccounts,
    #[msg("Invalid token account mint")]
    InvalidTokenAccountMint,
    #[msg("Invalid token account owner")]
    InvalidTokenAccountOwner,
    #[msg("Invalid royalty account owner")]
    InvalidRoyaltyAccountOwner,
    #[msg("Maker must be signer for zero salt orders")]
    MakerMustBeSignerForZeroSalt,
    #[msg("Collection bid orders must be submitted via the collection bid instruction")]
    CollectionBidMustUseCollectionBidInstruction,
    #[msg("Invalid Ed25519 instruction")]
    InvalidEd25519Instruction,
    #[msg("Collection bid price mismatch")]
    CollectionBidPriceMismatch,
    #[msg("Invalid collection bid taker collectionBid flag")]
    InvalidCollectionBidTakerFlag,
    #[msg("Collection bid taker make asset does not match")]
    CollectionBidTakerAssetMismatch,
    #[msg("Insufficient remaining accounts")]
    InsufficientRemainingAccounts,
    #[msg("Transfer failed")]
    TransferFailed,
}
