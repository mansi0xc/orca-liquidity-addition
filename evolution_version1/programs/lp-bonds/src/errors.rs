use anchor_lang::prelude::*;

/// Custom error codes for LP Bonds Level 1 Locker.
#[error_code]
pub enum LpBondsError {
    // =========================================================================
    // WHIRLPOOL VALIDATION ERRORS (6000-6009)
    // =========================================================================

    #[msg("Whirlpool address does not match allowlisted pool")]
    WhirlpoolNotAllowlisted,

    #[msg("Invalid Whirlpool program ID")]
    InvalidWhirlpoolProgram,

    #[msg("Token mint A does not match expected mint")]
    InvalidTokenMintA,

    #[msg("Token mint B does not match expected mint")]
    InvalidTokenMintB,

    #[msg("Token vault does not match whirlpool vault")]
    InvalidTokenVault,

    // =========================================================================
    // TICK VALIDATION ERRORS (6010-6019)
    // =========================================================================

    #[msg("Invalid tick range: lower must be less than upper")]
    InvalidTickRange,

    #[msg("Tick index out of bounds")]
    TickOutOfBounds,

    #[msg("Tick index not aligned to tick spacing")]
    TickNotAlignedToSpacing,

    // =========================================================================
    // TOKEN ACCOUNT ERRORS (6020-6029)
    // =========================================================================

    #[msg("Token account owner mismatch")]
    InvalidTokenOwner,

    #[msg("Token account mint mismatch")]
    InvalidTokenMint,

    #[msg("Invalid native mint address")]
    InvalidNativeMint,

    // =========================================================================
    // SOL HANDLING ERRORS (6030-6039)
    // =========================================================================

    #[msg("SOL amount must be greater than zero")]
    ZeroSolAmount,

    #[msg("Insufficient SOL balance")]
    InsufficientSolBalance,

    // =========================================================================
    // BOND NFT ERRORS (6040-6049)
    // =========================================================================

    #[msg("Bond mint mismatch")]
    InvalidBondMint,

    #[msg("Invalid bond balance: must hold exactly 1 bond NFT")]
    InvalidBondBalance,

    #[msg("Invalid bond metadata")]
    InvalidBondMetadata,

    // =========================================================================
    // POSITION CUSTODY ERRORS (6050-6059)
    // =========================================================================

    #[msg("Position mint does not match custody record")]
    InvalidPositionMint,

    #[msg("Custody bond mint mismatch")]
    InvalidCustodyBondMint,

    #[msg("Position NFT not found in custody")]
    PositionNftNotInCustody,

    #[msg("Invalid custody PDA")]
    InvalidCustodyPda,

    #[msg("Invalid position PDA")]
    InvalidPositionPda,

    // =========================================================================
    // AUTHORITY ERRORS (6060-6069)
    // =========================================================================

    #[msg("Unauthorized signer")]
    UnauthorizedSigner,

    #[msg("Invalid admin authority")]
    InvalidAdminAuthority,

    // =========================================================================
    // GENERAL ERRORS (6070-6079)
    // =========================================================================

    #[msg("Arithmetic overflow")]
    ArithmeticOverflow,

    #[msg("Invalid account data")]
    InvalidAccountData,

    #[msg("Operation failed")]
    OperationFailed,

    // =========================================================================
    // ORACLE VERIFICATION ERRORS (6080-6099)
    // =========================================================================

    #[msg("Invalid oracle signature")]
    InvalidOracleSignature,

    #[msg("Ed25519 instruction not found in transaction")]
    Ed25519InstructionNotFound,

    #[msg("Oracle authority does not match configured authority")]
    InvalidOracleAuthority,

    #[msg("Nonce already used - must be strictly greater than current")]
    NonceAlreadyUsed,

    #[msg("Nonce too old")]
    NonceTooOld,

    #[msg("Message reconstruction mismatch")]
    MessageReconstructionFailed,

    #[msg("Position data does not match custody record")]
    PositionDataMismatch,

    #[msg("Oracle already initialized")]
    OracleAlreadyInitialized,

    #[msg("Oracle not initialized")]
    OracleNotInitialized,

    #[msg("Invalid signature message length")]
    InvalidMessageLength,

    // =========================================================================
    // TIMELOCK ERRORS (6100-6109)
    // =========================================================================

    #[msg("Bond is still locked - cannot redeem before unlock time")]
    BondStillLocked,

    #[msg("Invalid lock duration")]
    InvalidLockDuration,
}
