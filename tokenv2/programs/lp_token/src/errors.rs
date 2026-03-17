use anchor_lang::prelude::error_code;

#[error_code]
pub enum LPTokenError {
    /// Caller is not the owner or a registered minter
    #[msg("LPToken: Only minter or owner is allowed")]
    Unauthorized,

    /// Mint or burn attempted while contract is paused
    #[msg("LPToken: Contract is paused")]
    Paused,

    /// Attempted to set a minter/pause state to its current value
    #[msg("LPToken: Duplicate operation")]
    DuplicateOperation,

    /// Token account does not belong to the expected authority
    #[msg("LPToken: Invalid token account authority")]
    InvalidTokenAuthority,

    /// The token account's mint does not match the program mint
    #[msg("LPToken: Token account mint mismatch")]
    InvalidMint,

    /// Pause/unpause called in the wrong state
    #[msg("LPToken: Invalid pause state transition")]
    InvalidPauseState,
}
