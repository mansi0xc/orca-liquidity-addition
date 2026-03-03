use anchor_lang::prelude::*;

/// Custom error codes for LP Bonds protocol.
/// Error codes 6000+ are custom Anchor errors.
#[error_code]
pub enum LpBondsError {
    // =========================================================================
    // WHIRLPOOL VALIDATION ERRORS (6000-6009)
    // =========================================================================

    /// The provided whirlpool does not match the allowlisted whirlpool.
    /// This prevents fake whirlpool injection attacks.
    #[msg("Whirlpool address does not match allowlisted pool")]
    WhirlpoolNotAllowlisted,

    /// The whirlpool program ID does not match expected Orca Whirlpool program.
    /// Prevents CPI to malicious programs masquerading as Whirlpool.
    #[msg("Invalid Whirlpool program ID")]
    InvalidWhirlpoolProgram,

    /// Token mint A does not match expected mint for this whirlpool.
    #[msg("Token mint A does not match expected mint")]
    InvalidTokenMintA,

    /// Token mint B does not match expected mint for this whirlpool.
    #[msg("Token mint B does not match expected mint")]
    InvalidTokenMintB,

    /// Token vault does not match whirlpool's configured vault.
    #[msg("Token vault does not match whirlpool vault")]
    InvalidTokenVault,

    // =========================================================================
    // TICK VALIDATION ERRORS (6010-6019)
    // =========================================================================

    /// Tick lower index must be less than tick upper index.
    #[msg("Invalid tick range: lower must be less than upper")]
    InvalidTickRange,

    /// Tick index is outside valid bounds.
    #[msg("Tick index out of bounds")]
    TickOutOfBounds,

    /// Tick index is not aligned to whirlpool tick spacing.
    #[msg("Tick index not aligned to tick spacing")]
    TickNotAlignedToSpacing,

    // =========================================================================
    // TOKEN ACCOUNT ERRORS (6020-6029)
    // =========================================================================

    /// Token account owner does not match expected owner.
    /// Prevents token account substitution attacks.
    #[msg("Token account owner mismatch")]
    InvalidTokenOwner,

    /// Token account mint does not match expected mint.
    #[msg("Token account mint mismatch")]
    InvalidTokenMint,

    /// Native mint address is incorrect.
    #[msg("Invalid native mint address")]
    InvalidNativeMint,

    // =========================================================================
    // SOL HANDLING ERRORS (6030-6039)
    // =========================================================================

    /// SOL amount must be greater than zero.
    #[msg("SOL amount must be greater than zero")]
    ZeroSolAmount,

    /// Insufficient SOL balance for wrapping.
    #[msg("Insufficient SOL balance")]
    InsufficientSolBalance,

    // =========================================================================
    // BOND NFT ERRORS (6040-6049)
    // =========================================================================

    /// Bond mint does not match expected mint.
    #[msg("Bond mint mismatch")]
    InvalidBondMint,

    /// Bond token account balance is not exactly 1.
    #[msg("Invalid bond balance: must hold exactly 1 bond NFT")]
    InvalidBondBalance,

    /// Bond metadata does not match expected format.
    #[msg("Invalid bond metadata")]
    InvalidBondMetadata,

    // =========================================================================
    // POSITION CUSTODY ERRORS (6050-6059)
    // =========================================================================

    /// Position mint does not match custody record.
    #[msg("Position mint does not match custody record")]
    InvalidPositionMint,

    /// Custody bond mint does not match provided bond mint.
    #[msg("Custody bond mint mismatch")]
    InvalidCustodyBondMint,

    /// Position NFT is not in custody.
    #[msg("Position NFT not found in custody")]
    PositionNftNotInCustody,

    /// Custody PDA derivation failed.
    #[msg("Invalid custody PDA")]
    InvalidCustodyPda,

    /// Position PDA does not match expected address.
    #[msg("Invalid position PDA")]
    InvalidPositionPda,

    // =========================================================================
    // AUTHORITY ERRORS (6060-6069)
    // =========================================================================

    /// Signer is not authorized for this operation.
    #[msg("Unauthorized signer")]
    UnauthorizedSigner,

    /// Admin authority mismatch.
    #[msg("Invalid admin authority")]
    InvalidAdminAuthority,

    // =========================================================================
    // GENERAL ERRORS (6070-6079)
    // =========================================================================

    /// Arithmetic overflow occurred.
    #[msg("Arithmetic overflow")]
    ArithmeticOverflow,

    /// Account data is invalid or corrupted.
    #[msg("Invalid account data")]
    InvalidAccountData,

    /// Operation failed due to unexpected state.
    #[msg("Operation failed")]
    OperationFailed,

    // =========================================================================
    // ORACLE VERIFICATION ERRORS (6080-6099)
    // =========================================================================

    /// Ed25519 signature verification failed.
    #[msg("Invalid oracle signature")]
    InvalidOracleSignature,

    /// Ed25519 program instruction not found in transaction.
    #[msg("Ed25519 instruction not found in transaction")]
    Ed25519InstructionNotFound,

    /// Oracle authority does not match configured authority.
    #[msg("Oracle authority does not match configured authority")]
    InvalidOracleAuthority,

    /// Nonce has already been used (replay attack prevention).
    #[msg("Nonce already used - must be strictly greater than current")]
    NonceAlreadyUsed,

    /// Nonce is too old (stale data protection).
    #[msg("Nonce too old")]
    NonceTooOld,

    /// Reconstructed message does not match signed message.
    #[msg("Message reconstruction mismatch")]
    MessageReconstructionFailed,

    /// Position data in signature does not match custody record.
    #[msg("Position data does not match custody record")]
    PositionDataMismatch,

    /// Oracle configuration already initialized.
    #[msg("Oracle already initialized")]
    OracleAlreadyInitialized,

    /// Oracle configuration not initialized.
    #[msg("Oracle not initialized")]
    OracleNotInitialized,

    /// Signature message has invalid length.
    #[msg("Invalid signature message length")]
    InvalidMessageLength,
}
