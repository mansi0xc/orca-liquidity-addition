use anchor_lang::prelude::*;

#[error_code]
pub enum LaunchpadError {
    #[msg("Arithmetic overflow occurred")]
    ArithmeticOverflow,
    #[msg("Arithmetic underflow occurred")]
    ArithmeticUnderflow,
    #[msg("Unauthorized access")]
    Unauthorized,
    #[msg("Collection is paused")]
    Paused,
    #[msg("Public sale is not active")]
    PublicsaleNotActive,
    #[msg("Presale is not active")]
    PresaleNotActive,
    #[msg("Zero quantity provided")]
    ZeroQuantity,
    #[msg("Exceeds max transaction limit")]
    MaxTxAmount,
    #[msg("Exceeds max user mint limit")]
    MaxUserAmount,
    #[msg("Exceeds max supply")]
    MaxSupply,
    #[msg("Not whitelisted")]
    NotWhitelisted,
    #[msg("All reserved NFTs have been minted")]
    ReservedNftsMinted,
    #[msg("Refund not supported for this collection type")]
    RefundNotSupported,
    #[msg("Invalid mint for this record")]
    InvalidMint,
    #[msg("Owner mints are not refundable")]
    OwnerMintNotRefundable,
    #[msg("Free NFTs are not refundable")]
    FreeNftNotRefundable,
    #[msg("Invalid token account")]
    InvalidTokenAccount,
    #[msg("Not the token owner")]
    NotTokenOwner,
    #[msg("Token account empty")]
    TokenAccountEmpty,
    #[msg("Insufficient vault balance")]
    InsufficientVaultBalance,
    #[msg("Name too long")]
    NameTooLong,
    #[msg("Symbol too long")]
    SymbolTooLong,
    #[msg("Base URI too long")]
    BaseUriTooLong,
    
    // New Errors for Production Security
    #[msg("Unsettled state: Token owner does not match TokenRecord owner. Operation blocked.")]
    UnsettledState,
    #[msg("Invalid nonce provided for transfer protection.")]
    InvalidNonce,
    #[msg("User mint cooldown rate limit exceeded.")]
    UserRateLimitExceeded,
    #[msg("Global collection mint cooldown rate limit exceeded.")]
    GlobalRateLimitExceeded,
    #[msg("Operator is not whitelisted in OperatorRegistry.")]
    OperatorNotWhitelisted,
    #[msg("Invalid sync path: token is already settled.")]
    AlreadySettled,
    #[msg("Invalid Operator PDA provided")]
    InvalidOperatorPda,
    #[msg("Price truncation error ensuring secure distribution")]
    PriceTruncationError,
}
