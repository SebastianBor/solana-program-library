use arrayref::{array_mut_ref, array_ref, array_refs, mut_array_refs};
use solana_program::{
    program_error::ProgramError,
    program_pack::{IsInitialized, Pack, Sealed},
};

use crate::state::enums::GovernanceAccountType;

/// Max instruction limit for generics
pub const INSTRUCTION_LIMIT: usize = 450;

/// Max accounts allowed in instruction
pub const MAX_ACCOUNTS_ALLOWED: usize = 12;

/// First iteration of generic instruction
#[derive(Clone)]
pub struct CustomSingleSignerTransaction {
    /// NOTE all Transaction structs MUST have slot as first u64 entry after account_type in byte buffer.
    /// Account type
    pub account_type: GovernanceAccountType,

    /// Slot waiting time between vote period ending and this being eligible for execution
    pub slot: u64,

    /// Instruction set
    pub instruction: [u8; INSTRUCTION_LIMIT],

    /// Executed flag
    pub executed: u8,

    /// Instruction end index (inclusive)
    pub instruction_end_index: u16,
}

impl PartialEq for CustomSingleSignerTransaction {
    fn eq(&self, other: &CustomSingleSignerTransaction) -> bool {
        if self.instruction.len() != other.instruction.len() {
            return false;
        }
        for n in 0..self.instruction.len() {
            if self.instruction[n] != other.instruction[n] {
                return false;
            }
        }
        self.slot == other.slot
    }
}

impl Sealed for CustomSingleSignerTransaction {}
impl IsInitialized for CustomSingleSignerTransaction {
    fn is_initialized(&self) -> bool {
        self.account_type != GovernanceAccountType::Uninitialized
    }
}
const CUSTOM_SINGLE_SIGNER_LEN: usize = 1 + 8 + INSTRUCTION_LIMIT + 1 + 2 + 300;
impl Pack for CustomSingleSignerTransaction {
    const LEN: usize = 1 + 8 + INSTRUCTION_LIMIT + 1 + 2 + 300;
    /// Unpacks a byte buffer into a transaction account
    fn unpack_from_slice(input: &[u8]) -> Result<Self, ProgramError> {
        let input = array_ref![input, 0, CUSTOM_SINGLE_SIGNER_LEN];
        #[allow(clippy::ptr_offset_with_cast)]
        let (account_type_value, slot, instruction, executed, instruction_end_index, _padding) =
            array_refs![input, 1, 8, INSTRUCTION_LIMIT, 1, 2, 300];

        let account_type = u8::from_le_bytes(*account_type_value);

        let account_type = match account_type {
            0 => GovernanceAccountType::Uninitialized,
            5 => GovernanceAccountType::CustomSingleSignerTransaction,
            _ => return Err(ProgramError::InvalidAccountData),
        };

        let slot = u64::from_le_bytes(*slot);
        let executed = u8::from_le_bytes(*executed);
        let instruction_end_index = u16::from_le_bytes(*instruction_end_index);

        Ok(CustomSingleSignerTransaction {
            account_type,
            slot,
            instruction: *instruction,
            executed,
            instruction_end_index,
        })
    }

    fn pack_into_slice(&self, output: &mut [u8]) {
        let output = array_mut_ref![output, 0, CUSTOM_SINGLE_SIGNER_LEN];
        #[allow(clippy::ptr_offset_with_cast)]
        let (account_type_value, slot, instruction, executed, instruction_end_index, _padding) =
            mut_array_refs![output, 1, 8, INSTRUCTION_LIMIT, 1, 2, 300];

        *account_type_value = match self.account_type {
            GovernanceAccountType::Uninitialized => 0_u8,
            GovernanceAccountType::CustomSingleSignerTransaction => 5_u8,
            _ => panic!("Account type was invalid"),
        }
        .to_le_bytes();

        *slot = self.slot.to_le_bytes();
        instruction.copy_from_slice(self.instruction.as_ref());
        *executed = self.executed.to_le_bytes();
        *instruction_end_index = self.instruction_end_index.to_le_bytes()
    }

    fn get_packed_len() -> usize {
        Self::LEN
    }

    fn unpack(input: &[u8]) -> Result<Self, ProgramError>
    where
        Self: IsInitialized,
    {
        let value = Self::unpack_unchecked(input)?;
        if value.is_initialized() {
            Ok(value)
        } else {
            Err(ProgramError::UninitializedAccount)
        }
    }

    fn unpack_unchecked(input: &[u8]) -> Result<Self, ProgramError> {
        if input.len() != Self::LEN {
            return Err(ProgramError::InvalidAccountData);
        }
        Self::unpack_from_slice(input)
    }

    fn pack(src: Self, dst: &mut [u8]) -> Result<(), ProgramError> {
        if dst.len() != Self::LEN {
            return Err(ProgramError::InvalidAccountData);
        }
        src.pack_into_slice(dst);
        Ok(())
    }
}