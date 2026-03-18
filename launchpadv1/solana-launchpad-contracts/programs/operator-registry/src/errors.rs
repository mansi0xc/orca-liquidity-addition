use anchor_lang::prelude::*;

#[error_code]
pub enum RegistryError {
    #[msg("Registry is paused")]
    RegistryPaused,

    #[msg("Operator is already whitelisted")]
    AlreadyWhitelisted,

    #[msg("Operator is not whitelisted")]
    NotWhitelisted,

    #[msg("Fund receiver already set to this address")]
    FundReceiverAlreadySet,

    #[msg("Invalid fund receiver address")]
    InvalidFundReceiver,

    #[msg("Share percentage exceeds maximum (10000 bps = 100%)")]
    SharePercentageTooHigh,

    #[msg("Unauthorized: caller is not the authority")]
    Unauthorized,
}
