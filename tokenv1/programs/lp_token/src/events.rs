use anchor_lang::prelude::*;

/// Emitted when the LP token mint is initialized.
#[event]
pub struct MintInitialized {
    pub mint: Pubkey,
    pub owner: Pubkey,
    pub evm_chain_id: u64,
    pub decimals: u8,
}

/// Emitted when tokens are minted.
/// Maps to EVM: Transfer(address(0), recipient, amount)
#[event]
pub struct TokensMinted {
    pub authority: Pubkey,
    pub recipient: Pubkey,
    pub amount: u64,
}

/// Emitted when tokens are burned.
/// Maps to EVM: Transfer(from, address(0), amount)
#[event]
pub struct TokensBurned {
    pub authority: Pubkey,
    pub from: Pubkey,
    pub amount: u64,
}

/// Emitted when a minter is registered or deregistered.
/// Maps to EVM: MinterUpdated(address indexed account, bool isMinter)
#[event]
pub struct MinterUpdated {
    pub minter: Pubkey,
    pub is_active: bool,
}

/// Emitted when the pause state changes.
/// Maps to EVM: Paused(address) / Unpaused(address)
#[event]
pub struct PauseStateChanged {
    pub paused: bool,
    pub authority: Pubkey,
}
