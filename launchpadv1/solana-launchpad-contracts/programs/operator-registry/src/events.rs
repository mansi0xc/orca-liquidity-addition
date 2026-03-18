use anchor_lang::prelude::*;

#[event]
pub struct OperatorWhitelistAdded {
    pub collection: Pubkey,
    pub operator: Pubkey,
}

#[event]
pub struct OperatorWhitelistRemoved {
    pub collection: Pubkey,
    pub operator: Pubkey,
}

#[event]
pub struct UniversalOperatorAdded {
    pub operator: Pubkey,
}

#[event]
pub struct UniversalOperatorRemoved {
    pub operator: Pubkey,
}

#[event]
pub struct FundReceiverChanged {
    pub old_receiver: Pubkey,
    pub new_receiver: Pubkey,
}

#[event]
pub struct SharePercentageBpsChanged {
    pub old_share_percentage: u64,
    pub new_share_percentage: u64,
}

#[event]
pub struct RegistryPauseToggled {
    pub paused: bool,
}
