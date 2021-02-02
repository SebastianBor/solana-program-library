const TRANSACTION_SLOTS: usize = 10;

const UNINITIALIZED_VERSION: u8 = 0;

/// Enums
pub mod enums;

/// custom single signer timelock transaction
pub mod custom_single_signer_timelock_transaction;
/// Timelock config
pub mod timelock_config;
/// Timelock program
pub mod timelock_program;
/// Timelock set
pub mod timelock_set;
/// Timelock state
pub mod timelock_state;
