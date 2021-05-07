use crate::state::enums::Vote;
use std::{convert::TryInto, mem::size_of};

use solana_program::{
    bpf_loader_upgradeable,
    epoch_schedule::Slot,
    instruction::{AccountMeta, Instruction},
    program_error::ProgramError,
    pubkey::Pubkey,
    system_program,
};

use crate::{
    error::GovernanceError,
    id,
    state::{
        custom_single_signer_transaction::MAX_INSTRUCTION_DATA,
        program_governance::GOVERNANCE_NAME_LENGTH,
        proposal_state::{DESC_SIZE, NAME_SIZE},
    },
};

/// Instructions supported by the Governance program.
#[derive(Clone)]
#[allow(clippy::large_enum_variant)]
pub enum GovernanceInstruction {
    /// Initializes a new empty Proposal for Instructions that will be executed at various slots in the future in draft mode.
    /// Grants Admin token to caller.
    ///
    ///   0. `[writable]` Uninitialized Proposal state account .
    ///   1. `[writable]` Uninitialized Proposal account .
    ///   2. `[writable]` Initialized Governance account.
    ///   3. `[writable]` Initialized Signatory Mint account
    ///   4. `[writable]` Initialized Admin Mint account
    ///   5. `[writable]` Initialized Voting Mint account
    ///   6. `[writable]` Initialized Yes Voting Mint account
    ///   7. `[writable]` Initialized No Voting Mint account
    ///   8. `[writable]` Initialized Signatory Validation account
    ///   9. `[writable]` Initialized Admin Validation account
    ///   10. `[writable]` Initialized Voting Validation account
    ///   11. `[writable]` Initialized Destination account for first admin token
    ///   12. `[writable]` Initialized Destination account for first signatory token
    ///   13. `[writable]` Initialized Yes voting dump account
    ///   14. `[writable]` Initialized No voting dump account
    ///   15. `[writable]` Initialized source holding account
    ///   16. `[]` Source mint
    ///   17. `[]` Governance minting authority (pda with seed of Proposal  key)
    ///   18. '[]` Token program id
    ///   19. `[]` Rent sysvar
    InitProposal {
        /// Link to gist explaining proposal
        /// UTF-8 encoded name of the proposal
        // TODO: Change to String
        description_link: [u8; DESC_SIZE],
        /// UTF-8 encoded name of the proposal
        // TODO: Change to String
        name: [u8; NAME_SIZE],
    },

    /// [Requires Admin token]
    /// Adds a signatory to the Proposal which means that this Proposal can't leave Draft state until yet another signatory burns
    /// their signatory token indicating they are satisfied with the instruction queue. They'll receive an signatory token
    /// as a result of this call that they can burn later.
    ///
    ///   0. `[writable]` Initialized new signatory account.
    ///   1. `[writable]` Initialized Signatory mint account.
    ///   2. `[writable]` Admin account.
    ///   3. `[writable]` Admin validation account.
    ///   5. `[writable]` Proposal state account.
    ///   6. `[]` Proposal account.
    ///   7. `[]` Transfer authority
    ///   8. `[]` Governance program mint authority (pda of seed with Proposal key)
    ///   9. '[]` Token program id.
    AddSignatory,

    /// [Requires Admin token]
    /// Removes a signer from the set.
    ///
    ///   0. `[writable]` Signatory account to remove token from.
    ///   1. `[writable]` Signatory mint account.
    ///   2. `[writable]` Admin account.
    ///   3. `[writable]` Admin validation account.
    ///   4. `[writable]` Proposal state account.
    ///   5. `[]` Proposal account.
    ///   6. `[]` Transfer authority
    ///   7. `[]` Governance program mint authority (pda of seed with Proposal key)
    ///   8. '[]` Token program id.
    RemoveSignatory,

    /// [Requires Signatory token]
    /// Adds a Transaction to the Proposal Max of 5 of any Transaction type. More than 5 will throw error.
    /// Creates a PDA using your authority to be used to later execute the instruction.
    /// This transaction needs to contain authority to execute the program.
    ///
    ///   0. `[writable]` Uninitialized Proposal Transaction account.
    ///   1. `[writable]` Proposal state account.
    ///   2. `[writable]` Signatory account
    ///   3. `[writable]` Signatory validation account.
    ///   4. `[]` Proposal account.
    ///   5. `[]` Governance account.
    ///   6. `[]` Transfer authority
    ///   7. `[]` Governance mint authority
    ///   8. `[]` Governance program account.
    ///   9. `[]` Token program account.
    AddCustomSingleSignerTransaction {
        /// Slot during which this will run
        delay_slots: u64,
        /// Instruction
        instruction: [u8; MAX_INSTRUCTION_DATA],
        /// Position in transaction array
        position: u8,
        /// Point in instruction array where 0 padding begins - inclusive, index should be where actual instruction ends, not where 0s begin
        instruction_end_index: u16,
    },

    /// [Requires Signatory token]
    /// Remove Transaction from the Proposal.
    ///
    ///   0. `[writable]` Proposal state account.
    ///   1. `[writable]` Proposal Transaction account.
    ///   2. `[writable]` Signatory account
    ///   3. `[writable]` Signatory validation account.
    ///   5. `[]` Proposal.
    ///   6. `[]` Transfer Authority.
    ///   7. `[]` Governance mint authority (pda of seed Proposal  key)
    ///   9. `[]` Token program account.
    RemoveTransaction,

    /// [Requires Signatory token]
    /// Update Transaction slot in the Proposal. Useful during reset periods.
    ///
    ///   1. `[writable]` Proposal Transaction account.
    ///   2. `[writable]` Signatory account
    ///   3. `[writable]` Signatory validation account.
    ///   4. `[]` Proposal state account.
    ///   5. `[]` Proposal account.
    ///   6. `[]` Transfer authority.
    ///   7. `[]` Governance mint authority (pda with seed of Proposal key)
    ///   8. `[]` Token program account.
    UpdateTransactionDelaySlots {
        /// On what slot this transaction slot will now run
        delay_slots: u64,
    },

    /// [Requires Admin token]
    /// Cancels Proposal by moving it into Cancelled state.
    ///
    ///   0. `[writable]` Proposal state account pub key.
    ///   1. `[writable]` Admin account
    ///   2. `[writable]` Admin validation account.
    ///   3. `[]` Proposal account pub key.
    ///   4. `[]` Transfer authority.
    ///   5. `[]` Governance mint authority (pda with seed of Proposal key)
    ///   6. `[]` Token program account.
    CancelProposal,

    /// [Requires Signatory token]
    /// Burns signatory token, indicating you approve of moving this Proposal from Draft state to Voting state.
    /// The last Signatory token to be burned moves the state to Voting.
    ///
    ///   0. `[writable]` Proposal state account pub key.
    ///   1. `[writable]` Signatory account
    ///   2. `[writable]` Signatory mint account.
    ///   3. `[]` Proposal account pub key.
    ///   4. `[]` Transfer authority
    ///   5. `[]` Governance mint authority (pda of seed Proposal key)ß
    ///   7. `[]` Token program account.
    ///   8. `[]` Clock sysvar.
    SignProposal,

    /// [Requires Voting tokens]
    /// Burns voting tokens, indicating you approve and/or disapprove of running this set of transactions. If you tip the consensus,
    /// then the transactions can begin to be run at their time slots when people click execute. You are then given yes and/or no tokens.
    ///
    ///   0. `[writable]` Governance voting record account.
    ///                   Can be uninitialized or initialized(if already used once in this proposal)
    ///                   Must have address with PDA having seed tuple [Governance acct key, proposal key, your voting account key]
    ///   1. `[writable]` Proposal state account.
    ///   2. `[writable]` Your Voting account.
    ///   3. `[writable]` Your Yes-Voting account.
    ///   4. `[writable]` Your No-Voting account.
    ///   5. `[writable]` Voting mint account.
    ///   6. `[writable]` Yes Voting mint account.
    ///   7. `[writable]` No Voting mint account.
    ///   8. `[]` Source mint account
    ///   9. `[]` Proposal account.
    ///   10. `[]` Governance account.
    ///   12. `[]` Transfer authority
    ///   13. `[]` Governance program mint authority (pda of seed Proposal key)
    ///   14. `[]` Token program account.
    ///   15. `[]` Clock sysvar.
    Vote {
        /// Casted vote
        vote: Vote,
    },

    /// Executes a command in the Proposal
    ///
    ///   0. `[writable]` Transaction account you wish to execute.
    ///   1. `[writable]` Proposal state account.
    ///   2. `[]` Program being invoked account
    ///   3. `[]` Proposal account.
    ///   4. `[]` Governance account
    ///   5. `[]` Governance program account pub key.
    ///   6. `[]` Clock sysvar.
    ///   7+ Any extra accounts that are part of the instruction, in order
    Execute,

    /// [Requires tokens of the Governance mint or Council mint depending on type of Proposal]
    /// Deposits voting tokens to be used during the voting process in a Proposal.
    /// These tokens are removed from your account and can be returned by withdrawing
    /// them from the Proposal (but then you will miss the vote.)
    ///
    ///   0. `[writable]` Governance voting record account. See Vote docs for more detail.
    ///   1. `[writable]` Initialized Voting account to hold your received voting tokens.
    ///   2. `[writable]` User token account to deposit tokens from.
    ///   3. `[writable]` Source holding account for Proposal that will accept the tokens in escrow.
    ///   4. `[writable]` Voting mint account.
    ///   5. `[]` Proposal account.
    ///   6. `[]` Transfer authority
    ///   7. `[]` Governance program mint authority (pda with seed of Proposal key)
    ///   8. `[]` Token program account.
    DepositSourceTokens {
        /// How many voting tokens to deposit
        voting_token_amount: u64,
    },

    /// [Requires voting tokens]
    /// Withdraws voting tokens.
    ///
    ///   0. `[writable]` Governance voting record account. See Vote docs for more detail.
    ///   1. `[writable]` Initialized Voting account from which to remove your voting tokens.
    ///   2. `[writable]` Initialized Yes Voting account from which to remove your voting tokens.
    ///   3. `[writable]` Initialized No Voting account from which to remove your voting tokens.
    ///   4. `[writable]` User token account that you wish your actual tokens to be returned to.
    ///   5. `[writable]` Source holding account owned by the Governance that will has the actual tokens in escrow.
    ///   6. `[writable]` Initialized Yes Voting dump account owned by Proposal to which to send your voting tokens.
    ///   7. `[writable]` Initialized No Voting dump account owned by Proposal to which to send your voting tokens.
    ///   8. `[writable]` Voting mint account.
    ///   9. `[writable]` Yes Voting mint account.
    ///   10. `[writable]` No Voting mint account.
    ///   11. `[]` Proposal state account.
    ///   12. `[]` Proposal account.
    ///   13. `[]` Transfer authority
    ///   14. `[]` Governance program mint authority (pda of seed Proposal key)
    ///   15. `[]` Token program account.
    WithdrawVotingTokens {
        /// How many voting tokens to withdrawal
        voting_token_amount: u64,
    },

    /// Creates Program Governance account
    ///
    ///   0. `[writable]` Governance account. The account pubkey needs to be set to program-derived address (PDA) with the following seeds:
    ///           1) 'governance' const prefix
    ///           2) Governed Program address
    ///   1. `[]` Account of the Program governed by this Governance account
    ///   2. `[writable]` Program Data account of the Program governed by this Governance account
    ///   3. `[signer]` Current Upgrade Authority account of the Program governed by this Governance account
    ///   4. `[]` Governance mint that this Governance uses
    ///   5. `[signer]` Payer
    ///   6. `[]` System account
    ///   7. `[]` Bpf_upgrade_loader account
    ///   8. `[]` Council mint that this Governance uses [Optional]
    CreateProgramGovernance {
        /// Voting threshold in % required to tip the vote
        /// It's the percentage of tokens out of the entire pool of governance tokens eligible to vote
        vote_threshold: u8,

        /// Minimum waiting time in slots for an instruction to be executed after proposal is voted on
        min_instruction_hold_up_time: Slot,

        /// Time limit in slots for proposal to be open to voting
        max_voting_time: Slot,

        /// Minimum % of tokens for a governance token owner to be able to create proposal
        /// It's the percentage of tokens out of the entire pool of governance tokens eligible to vote
        // TODO: Add field
        //token_threshold_to_create_proposal: u8,

        /// UTF-8 encoded Governance name
        // TODO: Change to String
        name: [u8; GOVERNANCE_NAME_LENGTH],
    },

    ///   0. `[]` Governance vote record key. Needs to be set with pubkey set to PDA with seeds of the
    ///           program account key, proposal key, your voting account key.
    ///   1. `[]` Proposal key
    ///   2. `[]` Your voting account
    ///   3. `[]` Payer
    ///   5. `[]` System account.
    CreateEmptyGovernanceVoteRecord,

    /// Creates Proposal Account
    CreateProposal {
        /// Link to gist explaining proposal
        /// UTF-8 encoded name of the proposal
        // TODO: Change to String
        description_link: [u8; DESC_SIZE],
        /// UTF-8 encoded name of the proposal
        // TODO: Change to String
        name: [u8; NAME_SIZE],
    },
}

impl GovernanceInstruction {
    /// Unpacks a byte buffer into a [GovernanceInstruction](enum.GovernanceInstruction.html).
    pub fn unpack(input: &[u8]) -> Result<Self, ProgramError> {
        let (&tag, rest) = input
            .split_first()
            .ok_or(GovernanceError::InstructionUnpackError)?;
        Ok(match tag {
            1 => {
                let (input_desc_link, input_name) = rest.split_at(DESC_SIZE);
                let mut desc_link = [0u8; DESC_SIZE];
                let mut name = [0u8; NAME_SIZE];

                desc_link[..(DESC_SIZE - 1)].clone_from_slice(&input_desc_link[..(DESC_SIZE - 1)]);
                name[..(NAME_SIZE - 1)].clone_from_slice(&input_name[..(NAME_SIZE - 1)]);
                Self::InitProposal {
                    description_link: desc_link,
                    name,
                }
            }
            2 => Self::AddSignatory,
            3 => Self::RemoveSignatory,
            4 => {
                let (delay_slots, rest) = Self::unpack_u64(rest)?;
                let (instruction, rest) = Self::unpack_instructions(rest)?;
                let (position, rest) = Self::unpack_u8(rest)?;
                let (instruction_end_index, _) = Self::unpack_u16(rest)?;
                Self::AddCustomSingleSignerTransaction {
                    delay_slots,
                    instruction,
                    position,
                    instruction_end_index,
                }
            }
            5 => Self::RemoveTransaction,
            6 => {
                let (delay_slots, _) = Self::unpack_u64(rest)?;
                Self::UpdateTransactionDelaySlots { delay_slots }
            }
            7 => Self::CancelProposal,
            8 => Self::SignProposal,
            9 => {
                let (yes_vote_amount, rest) = Self::unpack_u64(rest)?;
                let (no_vote_amount, _) = Self::unpack_u64(rest)?;

                let vote = if yes_vote_amount > 0 {
                    Vote::Yes(yes_vote_amount)
                } else if no_vote_amount > 0 {
                    Vote::No(no_vote_amount)
                } else {
                    return Err(GovernanceError::InstructionUnpackError.into());
                };

                Self::Vote { vote }
            }

            10 => {
                let (vote_threshold, rest) = Self::unpack_u8(rest)?;
                let (minimum_slot_waiting_period, rest) = Self::unpack_u64(rest)?;
                let (time_limit, rest) = Self::unpack_u64(rest)?;

                let mut name = [0u8; GOVERNANCE_NAME_LENGTH];
                name[..(GOVERNANCE_NAME_LENGTH - 1)]
                    .clone_from_slice(&rest[..(GOVERNANCE_NAME_LENGTH - 1)]);
                Self::CreateProgramGovernance {
                    vote_threshold,
                    min_instruction_hold_up_time: minimum_slot_waiting_period,
                    name,
                    max_voting_time: time_limit,
                }
            }
            11 => Self::Execute,
            12 => {
                let (voting_token_amount, _) = Self::unpack_u64(rest)?;
                Self::DepositSourceTokens {
                    voting_token_amount,
                }
            }
            13 => {
                let (voting_token_amount, _) = Self::unpack_u64(rest)?;
                Self::WithdrawVotingTokens {
                    voting_token_amount,
                }
            }
            14 => Self::CreateEmptyGovernanceVoteRecord,
            15 => {
                let (input_desc_link, input_name) = rest.split_at(DESC_SIZE);
                let mut desc_link = [0u8; DESC_SIZE];
                let mut name = [0u8; NAME_SIZE];

                desc_link[..(DESC_SIZE - 1)].clone_from_slice(&input_desc_link[..(DESC_SIZE - 1)]);
                name[..(NAME_SIZE - 1)].clone_from_slice(&input_name[..(NAME_SIZE - 1)]);
                Self::CreateProposal {
                    description_link: desc_link,
                    name,
                }
            }
            _ => return Err(GovernanceError::InstructionUnpackError.into()),
        })
    }

    fn unpack_u64(input: &[u8]) -> Result<(u64, &[u8]), ProgramError> {
        if input.len() >= 8 {
            let (amount, rest) = input.split_at(8);
            let amount = amount
                .get(..8)
                .and_then(|slice| slice.try_into().ok())
                .map(u64::from_le_bytes)
                .ok_or(GovernanceError::InstructionUnpackError)?;
            Ok((amount, rest))
        } else {
            Err(GovernanceError::InstructionUnpackError.into())
        }
    }

    fn unpack_u16(input: &[u8]) -> Result<(u16, &[u8]), ProgramError> {
        if input.len() >= 2 {
            let (amount, rest) = input.split_at(2);
            let amount = amount
                .get(..2)
                .and_then(|slice| slice.try_into().ok())
                .map(u16::from_le_bytes)
                .ok_or(GovernanceError::InstructionUnpackError)?;
            Ok((amount, rest))
        } else {
            Err(GovernanceError::InstructionUnpackError.into())
        }
    }

    fn unpack_instructions(
        input: &[u8],
    ) -> Result<([u8; MAX_INSTRUCTION_DATA], &[u8]), ProgramError> {
        if !input.is_empty() {
            if input.len() < MAX_INSTRUCTION_DATA {
                return Err(GovernanceError::InstructionUnpackError.into());
            }

            let (input_instruction, rest) = input.split_at(MAX_INSTRUCTION_DATA);
            let mut instruction = [0u8; MAX_INSTRUCTION_DATA];
            instruction[..(MAX_INSTRUCTION_DATA - 1)]
                .clone_from_slice(&input_instruction[..(MAX_INSTRUCTION_DATA - 1)]);
            Ok((instruction, rest))
        } else {
            Err(GovernanceError::InstructionUnpackError.into())
        }
    }

    fn unpack_u8(input: &[u8]) -> Result<(u8, &[u8]), ProgramError> {
        if !input.is_empty() {
            let (amount, rest) = input.split_at(1);
            let amount = amount
                .get(..1)
                .and_then(|slice| slice.try_into().ok())
                .map(u8::from_le_bytes)
                .ok_or(GovernanceError::InstructionUnpackError)?;
            Ok((amount, rest))
        } else {
            Err(GovernanceError::InstructionUnpackError.into())
        }
    }

    /// Packs a [GovernanceInstruction](enum.GovernanceInstruction.html) into a byte buffer.
    pub fn pack(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(size_of::<Self>());

        match self {
            Self::InitProposal {
                description_link: desc_link,
                name,
            } => {
                buf.push(1);
                buf.extend_from_slice(desc_link);
                buf.extend_from_slice(name);
            }
            Self::AddSignatory => buf.push(2),
            Self::RemoveSignatory => buf.push(3),
            Self::AddCustomSingleSignerTransaction {
                delay_slots,
                instruction,
                position,
                instruction_end_index,
            } => {
                buf.push(4);
                buf.extend_from_slice(&delay_slots.to_le_bytes());
                buf.extend_from_slice(instruction);
                buf.extend_from_slice(&position.to_le_bytes());
                buf.extend_from_slice(&instruction_end_index.to_le_bytes());
            }
            Self::RemoveTransaction {} => buf.push(5),
            Self::UpdateTransactionDelaySlots { delay_slots } => {
                buf.push(6);
                buf.extend_from_slice(&delay_slots.to_le_bytes());
            }
            Self::CancelProposal => buf.push(7),
            Self::SignProposal => buf.push(8),
            Self::Vote { vote } => {
                buf.push(9);

                let yes_vote_amount = match vote {
                    Vote::Yes(amount) => *amount,
                    _ => 0_u64,
                };

                let no_vote_amount = match vote {
                    Vote::No(amount) => *amount,
                    _ => 0,
                };

                buf.extend_from_slice(&yes_vote_amount.to_le_bytes());
                buf.extend_from_slice(&no_vote_amount.to_le_bytes());
            }
            Self::CreateProgramGovernance {
                vote_threshold,
                min_instruction_hold_up_time: minimum_slot_waiting_period,
                max_voting_time: time_limit,
                name,
            } => {
                buf.push(10);
                buf.extend_from_slice(&vote_threshold.to_le_bytes());
                buf.extend_from_slice(&minimum_slot_waiting_period.to_le_bytes());
                buf.extend_from_slice(&time_limit.to_le_bytes());
                buf.extend_from_slice(name);
            }
            Self::Execute => {
                buf.push(11);
            }
            Self::DepositSourceTokens {
                voting_token_amount,
            } => {
                buf.push(12);
                buf.extend_from_slice(&voting_token_amount.to_le_bytes());
            }
            Self::WithdrawVotingTokens {
                voting_token_amount,
            } => {
                buf.push(13);
                buf.extend_from_slice(&voting_token_amount.to_le_bytes());
            }
            Self::CreateEmptyGovernanceVoteRecord => buf.push(14),
            Self::CreateProposal {
                description_link,
                name,
            } => {
                buf.push(15);
                buf.extend_from_slice(description_link);
                buf.extend_from_slice(name);
            }
        }
        buf
    }
}

/// Creates CreateGovernance instruction
pub fn create_governance(
    governance_address: &Pubkey,
    governed_program_address: &Pubkey,
    governed_program_data_address: &Pubkey,
    governed_program_upgrade_authority: &Pubkey,
    governance_mint: &Pubkey,
    payer: &Pubkey,
    council_mint: &Option<Pubkey>,
    vote_threshold: u8,
    min_instruction_hold_up_time: u64,
    max_voting_time: u64,
    name: &[u8; GOVERNANCE_NAME_LENGTH],
) -> Result<Instruction, ProgramError> {
    let mut accounts = vec![
        AccountMeta::new(*governance_address, false),
        AccountMeta::new_readonly(*governed_program_address, false),
        AccountMeta::new(*governed_program_data_address, false),
        AccountMeta::new_readonly(*governed_program_upgrade_authority, true),
        AccountMeta::new_readonly(*governance_mint, false),
        AccountMeta::new_readonly(*payer, true),
        AccountMeta::new_readonly(system_program::id(), false),
        AccountMeta::new_readonly(bpf_loader_upgradeable::id(), false),
    ];

    if let Some(council_mint_key) = council_mint {
        accounts.push(AccountMeta::new_readonly(*council_mint_key, false));
    }

    let instruction = GovernanceInstruction::CreateProgramGovernance {
        vote_threshold,
        min_instruction_hold_up_time,
        max_voting_time,
        name: *name,
    };

    Ok(Instruction {
        program_id: id(),
        accounts,
        data: instruction.pack(),
    })
}

/// Creates proposal
pub fn create_proposal(
    description_link: &[u8; DESC_SIZE],
    name: &[u8; NAME_SIZE],
    payer: &Pubkey,
) -> Result<Instruction, ProgramError> {
    let accounts = vec![
        AccountMeta::new_readonly(*payer, true),
        AccountMeta::new_readonly(system_program::id(), false),
    ];

    let instruction = GovernanceInstruction::CreateProposal {
        description_link: *description_link,
        name: *name,
    };

    Ok(Instruction {
        program_id: id(),
        accounts,
        data: instruction.pack(),
    })
}
