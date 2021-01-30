pub mod process_init_timelock_program;
pub mod process_init_timelock_set;

use crate::instruction::TimelockInstruction;
use process_init_timelock_program::process_init_timelock_program;
use process_init_timelock_set::process_init_timelock_set;

use solana_program::{account_info::AccountInfo, entrypoint::ProgramResult, msg, pubkey::Pubkey};

/// Processes an instruction
pub fn process_instruction(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    input: &[u8],
) -> ProgramResult {
    let instruction = TimelockInstruction::unpack(input)?;
    match instruction {
        TimelockInstruction::InitTimelockProgram => {
            msg!("Instruction: Init Timelock Program");
            process_init_timelock_program(program_id, accounts)
        }
        TimelockInstruction::InitTimelockSet { config } => {
            msg!("Instruction: Init Timelock Set");
            process_init_timelock_set(program_id, accounts)
        }
        TimelockInstruction::AddSigner => Ok(()),
        TimelockInstruction::RemoveSigner => Ok(()),
        TimelockInstruction::AddCustomSingleSignerV1Transaction { slot, instruction } => Ok(()),
        TimelockInstruction::RemoveTransaction {} => Ok(()),
        TimelockInstruction::UpdateTransactionSlot { slot } => Ok(()),
        TimelockInstruction::DeleteTimelockSet {} => Ok(()),
        TimelockInstruction::Sign {} => Ok(()),
        TimelockInstruction::Vote {
            voting_token_amount,
        } => Ok(()),
        TimelockInstruction::MintVotingTokens {
            voting_token_amount,
        } => Ok(()),
    }
}