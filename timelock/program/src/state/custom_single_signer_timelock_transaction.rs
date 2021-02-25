use arrayref::{array_mut_ref, array_ref, array_refs, mut_array_refs};
use solana_program::{
    program_error::ProgramError,
    program_pack::{IsInitialized, Pack, Sealed},
    pubkey::Pubkey,
};

use super::UNINITIALIZED_VERSION;

/// Max instruction limit for generics
pub const INSTRUCTION_LIMIT: usize = 255;

/// STRUCT VERSION
pub const CUSTOM_SINGLE_SIGNER_TIMELOCK_TRANSACTION_VERSION: u8 = 1;

/// First iteration of generic instruction
#[derive(Clone)]
pub struct CustomSingleSignerTimelockTransaction {
    /// NOTE all Transaction structs MUST have slot as first u64 entry in byte buffer.

    /// version
    pub version: u8,

    /// Slot at which this will execute
    pub slot: u64,

    /// Instruction set
    pub instruction: [u8; INSTRUCTION_LIMIT],

    /// authority key (pda) used to run the program
    pub authority_key: Pubkey,
}

impl PartialEq for CustomSingleSignerTimelockTransaction {
    fn eq(&self, other: &CustomSingleSignerTimelockTransaction) -> bool {
        if self.instruction.len() != other.instruction.len() {
            return false;
        }
        for n in 0..self.instruction.len() {
            if self.instruction[n] != other.instruction[n] {
                return false;
            }
        }
        self.slot == other.slot && self.authority_key.to_bytes() == other.authority_key.to_bytes()
    }
}

impl Sealed for CustomSingleSignerTimelockTransaction {}
impl IsInitialized for CustomSingleSignerTimelockTransaction {
    fn is_initialized(&self) -> bool {
        self.version != UNINITIALIZED_VERSION
    }
}
const CUSTOM_SINGLE_SIGNER_LEN: usize = 1 + 8 + INSTRUCTION_LIMIT + 32;
impl Pack for CustomSingleSignerTimelockTransaction {
    const LEN: usize = 1 + 8 + INSTRUCTION_LIMIT + 32;
    /// Unpacks a byte buffer into a [TimelockProgram](struct.TimelockProgram.html).
    fn unpack_from_slice(input: &[u8]) -> Result<Self, ProgramError> {
        let input = array_ref![input, 0, CUSTOM_SINGLE_SIGNER_LEN];
        #[allow(clippy::ptr_offset_with_cast)]
        let (version, slot, instruction, authority_key) =
            array_refs![input, 1, 8, INSTRUCTION_LIMIT, 32];
        let version = u8::from_le_bytes(*version);
        let slot = u64::from_le_bytes(*slot);
        let authority_key = Pubkey::new_from_array(*authority_key);

        Ok(CustomSingleSignerTimelockTransaction {
            version,
            slot,
            instruction: *instruction,
            authority_key,
        })
    }

    fn pack_into_slice(&self, output: &mut [u8]) {
        let output = array_mut_ref![output, 0, CUSTOM_SINGLE_SIGNER_LEN];
        #[allow(clippy::ptr_offset_with_cast)]
        let (version, slot, instruction, authority_key) =
            mut_array_refs![output, 1, 8, INSTRUCTION_LIMIT, 32];
        *version = self.version.to_le_bytes();
        *slot = self.slot.to_le_bytes();
        instruction.copy_from_slice(self.instruction.as_ref());
        authority_key.copy_from_slice(self.authority_key.as_ref());
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
        Ok(Self::unpack_from_slice(input)?)
    }

    fn pack(src: Self, dst: &mut [u8]) -> Result<(), ProgramError> {
        if dst.len() != Self::LEN {
            return Err(ProgramError::InvalidAccountData);
        }
        src.pack_into_slice(dst);
        Ok(())
    }
}