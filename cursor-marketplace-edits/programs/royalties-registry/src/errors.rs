use anchor_lang::prelude::*;

#[error_code]
pub enum RegistryError {
    #[msg("Unauthorized: not registry owner or collection authority")]
    Unauthorized,
    #[msg("Royalty recipient cannot be zero address")]
    ZeroAddressRecipient,
    #[msg("Total royalties cannot exceed 100%")]
    RoyaltiesTooHigh,
    #[msg("Too many royalty recipients")]
    TooManyRecipients,
    #[msg("Not the token creator")]
    NotTokenCreator,
}
