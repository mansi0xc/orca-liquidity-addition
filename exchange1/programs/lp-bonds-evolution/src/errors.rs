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

    #[msg("Invalid tick range: lower must be less than upper")]
    InvalidTickRange,

    #[msg("Tick index out of bounds")]
    TickOutOfBounds,

    #[msg("Tick index not aligned to tick spacing")]
    TickNotAlignedToSpacing,

    #[msg("Oracle tick_current does not match whirlpool on-chain tick_current_index")]
    TickCurrentMismatch,

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

    #[msg("Invalid pending admin")]
    InvalidPendingAdmin,

    #[msg("No pending admin set")]
    NoPendingAdmin,

    #[msg("Unauthorized - insufficient permissions")]
    InsufficientPermissions,

    // =========================================================================
    // ORACLE ERRORS (6050-6069)
    // =========================================================================

    #[msg("Invalid oracle signature data in Ed25519 instruction")]
    InvalidOracleSignature,

    #[msg("Ed25519 instruction not found at the expected transaction index")]
    Ed25519InstructionNotFound,

    #[msg("Oracle authority does not match configured authority")]
    InvalidOracleAuthority,

    #[msg("Nonce must be exactly current_nonce + 1 (strict sequential)")]
    InvalidNonceSequence,

    #[msg("Message in Ed25519 instruction does not match reconstructed message")]
    MessageReconstructionFailed,

    #[msg("Message length in Ed25519 instruction does not match expected EVOLUTION_CANONICAL_MESSAGE_LEN")]
    InvalidMessageLength,

    #[msg("Oracle timestamp is stale — exceeds MAX_ORACLE_STALENESS_SECONDS")]
    OracleTimestampStale,

    #[msg("Oracle timestamp is in the future")]
    OracleTimestampFuture,

    #[msg("Ed25519 instruction must be immediately before this instruction (strict ordering)")]
    InvalidInstructionOrder,

    #[msg("Failed to load current instruction index from instructions sysvar")]
    InvalidEd25519Index,

    #[msg("Position data does not match expected whirlpool after CPI")]
    PositionDataMismatch,

    #[msg("Evolution rate limited — minimum delay between evolutions not met")]
    EvolutionRateLimited,

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

    #[msg("Mint cap exceeded - total minted would exceed max_total_mint")]
    MintCapExceeded,

    #[msg("Invalid mint cap - max_total_mint must be greater than zero")]
    InvalidMintCap,

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

    // =========================================================================
    // INPUT VALIDATION ERRORS (6100-6109)
    // =========================================================================

    #[msg("Liquidity amount must be greater than zero")]
    ZeroLiquidityAmount,

    #[msg("At least one token amount must be greater than zero")]
    ZeroTokenAmounts,

    #[msg("Tick array PDA does not match expected derivation from whirlpool + start_tick_index")]
    InvalidTickArrayPda,

    // =========================================================================
    // TIMELOCK ERRORS (6110-6119)
    // =========================================================================

    #[msg("Bond is still locked - cannot redeem before unlock time")]
    BondStillLocked,

    #[msg("Invalid lock duration - must be greater than zero")]
    InvalidLockDuration,

    // =========================================================================
    // ORACLE ENABLED ERRORS (6120-6129)
    // =========================================================================

    #[msg("Oracle is not enabled")]
    OracleNotEnabled,

    // =========================================================================
    // RECOVERY ERRORS (6130-6139)
    // =========================================================================

    #[msg("Cannot recover tokens from active custody account")]
    RecoveryCustodyProtected,
}
