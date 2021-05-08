//! RootGovernance Account

use super::enums::GovernanceAccountType;

use borsh::{BorshDeserialize, BorshSchema, BorshSerialize};

use solana_program::{program_pack::IsInitialized, pubkey::Pubkey};

/// Governance Proposal
#[derive(Clone, Debug, PartialEq, BorshDeserialize, BorshSerialize, BorshSchema)]
pub struct RootGovernance {
    /// Governance account type
    pub account_type: GovernanceAccountType,

    /// Governance mint
    pub governance_mint: Pubkey,

    /// Council mint
    pub council_mint: Option<Pubkey>,

    /// Governance name
    pub name: String,
}

impl IsInitialized for RootGovernance {
    fn is_initialized(&self) -> bool {
        self.account_type == GovernanceAccountType::RootGovernance
    }
}