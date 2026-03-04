use anchor_lang::prelude::*;

/// Custom error codes for LP Bonds Evolution Program.
#[error_code]
pub enum EvolutionError {
    // =========================================================================
    // EVOLUTION ERRORS (6000-6019)
    // =========================================================================

    #[msg("Evolution is paused")]
    EvolutionPaused,

    #[msg("Evolution not initialized")]
    EvolutionNotInitialized,

    #[msg("Evolution already initialized")]
    EvolutionAlreadyInitialized,

    #[msg("Invalid bond level - must be 1-4")]
    InvalidBondLevel,

    #[msg("Invalid level transition - must evolve to next level")]
    InvalidLevelTransition,

    #[msg("Level configuration not found")]
    LevelConfigNotFound,

    #[msg("Level is not active for evolution")]
    LevelNotActive,

    #[msg("Bond has already been evolved")]
    BondAlreadyEvolved,

    #[msg("Source bond level does not match")]
    SourceLevelMismatch,

    #[msg("Whirlpool does not match expected whirlpool for level")]
    WhirlpoolLevelMismatch,

    #[msg("Insufficient token amount for evolution")]
    InsufficientEvolutionAmount,

    #[msg("Invalid evolution signature")]
    InvalidEvolutionSignature,

    #[msg("Evolution oracle authority does not match")]
    InvalidEvolutionOracle,

    #[msg("Maximum bond level reached - cannot evolve further")]
    MaxLevelReached,

    // =========================================================================
    // VALIDATION ERRORS (6020-6039)
    // =========================================================================

    #[msg("Invalid tick range")]
    InvalidTickRange,

    #[msg("Tick index out of bounds")]
    TickOutOfBounds,

    #[msg("Token account owner mismatch")]
    InvalidTokenOwner,

    #[msg("Token account mint mismatch")]
    InvalidTokenMint,

    #[msg("Bond mint mismatch")]
    InvalidBondMint,

    #[msg("Invalid bond balance")]
    InvalidBondBalance,

    #[msg("Position mint mismatch")]
    InvalidPositionMint,

    #[msg("Custody bond mint mismatch")]
    InvalidCustodyBondMint,

    #[msg("Invalid custody PDA")]
    InvalidCustodyPda,

    #[msg("Invalid position PDA")]
    InvalidPositionPda,

    #[msg("Invalid Whirlpool program ID")]
    InvalidWhirlpoolProgram,

    #[msg("Invalid token vault")]
    InvalidTokenVault,

    // =========================================================================
    // AUTHORITY ERRORS (6040-6049)
    // =========================================================================

    #[msg("Invalid admin authority")]
    InvalidAdminAuthority,

    #[msg("Unauthorized signer")]
    UnauthorizedSigner,

    // =========================================================================
    // ORACLE ERRORS (6050-6069)
    // =========================================================================

    #[msg("Invalid oracle signature")]
    InvalidOracleSignature,

    #[msg("Ed25519 instruction not found")]
    Ed25519InstructionNotFound,

    #[msg("Invalid oracle authority")]
    InvalidOracleAuthority,

    #[msg("Nonce already used")]
    NonceAlreadyUsed,

    #[msg("Message reconstruction failed")]
    MessageReconstructionFailed,

    // =========================================================================
    // FEE ERRORS (6070-6079)
    // =========================================================================

    #[msg("Fee exceeds maximum allowed")]
    FeeTooHigh,

    #[msg("Invalid fee configuration")]
    InvalidFeeConfig,

    #[msg("Treasury address not set")]
    TreasuryNotSet,

    // =========================================================================
    // LAYER TOKEN ERRORS (6080-6089)
    // =========================================================================

    #[msg("Layer token mint not configured")]
    LayerTokenNotConfigured,

    #[msg("Invalid layer token mint authority")]
    InvalidLayerTokenAuthority,

    #[msg("Layer token mint failed")]
    LayerTokenMintFailed,

    // =========================================================================
    // GENERAL ERRORS (6090-6099)
    // =========================================================================

    #[msg("Arithmetic overflow")]
    ArithmeticOverflow,

    #[msg("Invalid account data")]
    InvalidAccountData,

    #[msg("Operation failed")]
    OperationFailed,

    #[msg("Insufficient remaining accounts")]
    InsufficientRemainingAccounts,
}
