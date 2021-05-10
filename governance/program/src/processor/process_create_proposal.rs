//! Program state processor

use solana_program::{
    account_info::{next_account_info, AccountInfo},
    entrypoint::ProgramResult,
    pubkey::Pubkey,
};

use crate::{
    state::{
        enums::GovernanceAccountType, program_governance::ProgramGovernance, proposal::Proposal,
    },
    tools::account::create_and_serialize_account,
    utils::deserialize_account,
};

/// process_create_proposal
pub fn process_create_proposal(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    description_link: String,
    name: String,
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();

    let proposal_info = next_account_info(account_info_iter)?; // 1
    let governance_info = next_account_info(account_info_iter)?; // 2
    let payer_info = next_account_info(account_info_iter)?; // 3
    let system_info = next_account_info(account_info_iter)?; // 4

    let mut _governance: ProgramGovernance = deserialize_account(governance_info, program_id)?;

    let proposal_data = Proposal {
        account_type: GovernanceAccountType::Proposal,
        name,
        description_link,
    };

    create_and_serialize_account::<Proposal>(
        payer_info,
        proposal_info,
        &proposal_data,
        program_id,
        system_info,
    )?;

    Ok(())
}
