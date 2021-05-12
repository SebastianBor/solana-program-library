//! VoterRecord Account

use crate::{
    error::GovernanceError, id, tools::account::deserialize_account, PROGRAM_AUTHORITY_SEED,
};

use super::enums::{GovernanceAccountType, GoverningTokenType};

use borsh::{BorshDeserialize, BorshSchema, BorshSerialize};

use solana_program::{
    account_info::AccountInfo, program_error::ProgramError, program_pack::IsInitialized,
    pubkey::Pubkey,
};

/// Governance Voter Record
/// Account PDA seeds: ['governance', realm, token_mint, token_owner ]
#[derive(Clone, Debug, PartialEq, BorshDeserialize, BorshSerialize, BorshSchema)]
pub struct VoterRecord {
    /// Governance account type
    pub account_type: GovernanceAccountType,

    /// The Realm the VoterRecord belongs to
    pub realm: Pubkey,

    /// The type of the Governing Token the VoteRecord is for
    pub token_type: GoverningTokenType,

    /// The owner (either single or multisig) of the deposited governing SPL Tokens
    /// This is who can authorize a withdrawal
    pub token_owner: Pubkey,

    /// The amount of governing tokens deposited into the Realm
    /// This amount is the voter weight used when voting on proposals
    pub token_deposit_amount: u64,

    /// A single account that is allowed to operate governance with the deposited governing tokens
    /// It's delegated to by the token owner
    pub vote_authority: Pubkey,

    /// The number of active votes cast by voter
    pub active_votes_count: u8,

    /// The total number of votes cast by the voter
    pub total_votes_count: u8,
}

impl IsInitialized for VoterRecord {
    fn is_initialized(&self) -> bool {
        self.account_type == GovernanceAccountType::VoterRecord
    }
}

pub fn get_voter_record_address(
    realm: &Pubkey,
    governing_token_mint: &Pubkey,
    governing_token_owner: &Pubkey,
) -> Pubkey {
    Pubkey::find_program_address(
        &get_voter_record_address_seeds(realm, governing_token_mint, governing_token_owner)[..],
        &id(),
    )
    .0
}

pub fn get_voter_record_address_seeds<'a>(
    realm: &'a Pubkey,
    governing_token_mint: &'a Pubkey,
    governing_token_owner: &'a Pubkey,
) -> Vec<&'a [u8]> {
    vec![
        PROGRAM_AUTHORITY_SEED,
        realm.as_ref(),
        governing_token_mint.as_ref(),
        governing_token_owner.as_ref(),
    ]
}

pub fn deserialize_voter_record(
    voter_record_info: &AccountInfo,
    voter_record_seeds: Vec<&[u8]>,
) -> Result<VoterRecord, ProgramError> {
    let (voter_record_address, _) = Pubkey::find_program_address(&voter_record_seeds[..], &id());

    if voter_record_address != *voter_record_info.key {
        return Err(GovernanceError::InvalidVoterAccountAddress.into());
    }

    Ok(deserialize_account::<VoterRecord>(
        voter_record_info,
        &id(),
    )?)
}
