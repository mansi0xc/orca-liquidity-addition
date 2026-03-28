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

    #[msg("Oracle tick_current does not match whirlpool on-chain tick_current_index")]
    TickCurrentMismatch,

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

    #[msg("Invalid pending admin")]
    InvalidPendingAdmin,

    #[msg("No pending admin set")]
    NoPendingAdmin,

    // =========================================================================
    // GENERAL ERRORS (6070-6079)
    // =========================================================================

    #[msg("Arithmetic overflow")]
    ArithmeticOverflow,

    #[msg("Invalid account data")]
    InvalidAccountData,

    #[msg("Operation failed")]
    OperationFailed,

    #[msg("Protocol is paused")]
    ProtocolPaused,

    // =========================================================================
    // ORACLE VERIFICATION ERRORS (6080-6099)
    // =========================================================================

    #[msg("Invalid oracle signature data in Ed25519 instruction")]
    InvalidOracleSignature,

    #[msg("Ed25519 instruction not found at the expected transaction index")]
    Ed25519InstructionNotFound,

    #[msg("Oracle authority does not match configured authority")]
    InvalidOracleAuthority,

    #[msg("Nonce must be exactly current_nonce + 1 (strict sequential)")]
    InvalidNonceSequence,

    #[msg("Nonce too old")]
    NonceTooOld,

    #[msg("Message in Ed25519 instruction does not match reconstructed message")]
    MessageReconstructionFailed,

    #[msg("Position data does not match custody record")]
    PositionDataMismatch,

    #[msg("Oracle already initialized")]
    OracleAlreadyInitialized,

    #[msg("Oracle not initialized")]
    OracleNotInitialized,

    #[msg("Message length in Ed25519 instruction does not match expected ORACLE_MESSAGE_LEN")]
    InvalidMessageLength,

    #[msg("Oracle timestamp is stale — exceeds MAX_ORACLE_STALENESS_SECONDS")]
    OracleTimestampStale,

    #[msg("Oracle timestamp is in the future")]
    OracleTimestampFuture,

    #[msg("Oracle is not enabled")]
    OracleNotEnabled,

    #[msg("Ed25519 instruction must be immediately before this instruction (strict ordering)")]
    InvalidInstructionOrder,

    #[msg("Failed to load current instruction index from instructions sysvar")]
    InvalidEd25519Index,

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

    #[msg("Invalid lock duration")]
    InvalidLockDuration,

    // =========================================================================
    // RECOVERY ERRORS (6120-6129)
    // =========================================================================

    #[msg("Cannot recover tokens from active custody account")]
    RecoveryCustodyProtected,

    // =========================================================================
    // EXCHANGE ERRORS (6130-6139)
    // =========================================================================

    #[msg("Exchange is not active")]
    ExchangeNotActive,

    #[msg("Exchange already initialized")]
    ExchangeAlreadyInitialized,

    #[msg("Exchange output token mint mismatch")]
    InvalidExchangeTokenMint,

    #[msg("Exchange nonce already used")]
    ExchangeNonceAlreadyUsed,
}
