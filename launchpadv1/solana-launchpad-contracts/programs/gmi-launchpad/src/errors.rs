use anchor_lang::prelude::*;

/// Custom errors for the GMI Launchpad program.
/// Maps to all EVM require conditions (R1-R18) plus Solana-specific checks.
#[error_code]
pub enum LaunchpadError {
    // === Sale Status Errors ===
    #[msg("Public sale is not active")]
    PublicsaleNotActive,         // R1

    #[msg("Presale is not active")]
    PresaleNotActive,            // R2

    #[msg("Collection is paused")]
    Paused,

    // === Whitelist Errors ===
    #[msg("User is not whitelisted")]
    NotWhitelisted,              // R3

    #[msg("Whitelist entry already exists")]
    WhitelistAlreadyExists,

    // === Payment Errors ===
    #[msg("Incorrect payment amount")]
    BadValue,                    // R4 / R18

    // === Quantity / Limit Errors ===
    #[msg("Quantity exceeds maximum per transaction")]
    MaxTxAmount,                 // R5

    #[msg("User has reached maximum mint amount")]
    MaxUserAmount,               // R6

    #[msg("Maximum supply reached")]
    MaxSupply,                   // R7

    #[msg("Quantity must be greater than zero")]
    ZeroQuantity,

    // === Refund Errors ===
    #[msg("Collection type does not support refunds")]
    RefundNotSupported,

    #[msg("Cannot refund owner-minted tokens")]
    OwnerMintNotRefundable,      // R10

    #[msg("Cannot refund free NFTs (refund price is zero)")]
    FreeNftNotRefundable,        // R11

    #[msg("Caller does not own this token")]
    NotTokenOwner,               // R9

    #[msg("Token account is empty")]
    TokenAccountEmpty,

    // === Reserved Mint Errors ===
    #[msg("All reserved NFTs have been minted")]
    ReservedNftsMinted,          // R15

    // === Authority Errors ===
    #[msg("Unauthorized: caller is not the authority")]
    Unauthorized,

    #[msg("Invalid user address")]
    InvalidUserAddress,          // R13

    // === Arithmetic Errors ===
    #[msg("Arithmetic overflow")]
    ArithmeticOverflow,

    #[msg("Arithmetic underflow")]
    ArithmeticUnderflow,

    // === Vault Errors ===
    #[msg("Insufficient vault balance for refund")]
    InsufficientVaultBalance,

    #[msg("Transfer failed")]
    TransferFailed,              // R16

    // === Account Validation ===
    #[msg("Invalid collection type for this operation")]
    InvalidCollectionType,

    #[msg("Invalid mint account")]
    InvalidMint,

    #[msg("Invalid token account")]
    InvalidTokenAccount,

    #[msg("Base URI too long (max 200 characters)")]
    BaseUriTooLong,

    #[msg("Name too long (max 32 characters)")]
    NameTooLong,

    #[msg("Symbol too long (max 10 characters)")]
    SymbolTooLong,
}
