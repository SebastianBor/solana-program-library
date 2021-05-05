use std::{env, fs::File, io::Read, path::PathBuf};

use solana_program::{
    bpf_loader_upgradeable::{self, UpgradeableLoaderState},
    instruction::Instruction,
    program_pack::Pack,
    pubkey::Pubkey,
    rent::Rent,
};
use solana_program_test::ProgramTest;
use solana_program_test::*;

use solana_sdk::{
    hash::Hash,
    signature::{Keypair, Signer},
    transaction::Transaction,
};
use spl_governance::{
    id,
    instruction::{create_dummy_account, create_governance},
    processor::process_instruction,
    state::governance::{Governance, GOVERNANCE_NAME_LENGTH},
    PROGRAM_AUTHORITY_SEED,
};

pub struct GovernedProgramSetup {
    pub address: Pubkey,
    pub upgrade_authority: Keypair,
    pub data_address: Pubkey,
}

pub struct GovernanceSetup {
    pub address: Pubkey,
    pub governance_mint: Pubkey,
    pub council_mint: Option<Pubkey>,
    pub vote_threshold: u8,
    pub minimum_slot_waiting_period: u64,
    pub time_limit: u64,
    pub name: [u8; GOVERNANCE_NAME_LENGTH],
}

pub struct GovernanceProgramTest {
    pub banks_client: BanksClient,
    pub payer: Keypair,
    pub recent_blockhash: Hash,
    pub rent: Rent,
}

impl GovernanceProgramTest {
    pub async fn start_new() -> Self {
        let mut program_test = ProgramTest::new(
            "spl_governance",
            spl_governance::id(),
            processor!(process_instruction),
        );

        program_test.add_program(
            "solana_bpf_loader_upgradeable_program",
            bpf_loader_upgradeable::id(),
            Some(solana_bpf_loader_program::process_instruction),
        );

        let (mut banks_client, payer, recent_blockhash) = program_test.start().await;

        let rent = banks_client.get_rent().await.unwrap();

        Self {
            banks_client,
            payer,
            recent_blockhash,
            rent,
        }
    }

    async fn process_transaction(
        &mut self,
        instructions: &[Instruction],
        signers: Option<&[&Keypair]>,
    ) {
        let mut transaction =
            Transaction::new_with_payer(&instructions, Some(&self.payer.pubkey()));

        let mut all_signers = vec![&self.payer];

        if let Some(signers) = signers {
            all_signers.extend_from_slice(signers);
        }

        transaction.sign(&all_signers, self.recent_blockhash);

        self.banks_client
            .process_transaction(transaction)
            .await
            .unwrap();
    }

    pub async fn with_governed_program(&mut self) -> GovernedProgramSetup {
        let program_address_keypair = Keypair::new();
        let program_buffer_keypair = Keypair::new();
        let program_upgrade_authority_keypair = Keypair::new();

        let (program_data_address, _) = Pubkey::find_program_address(
            &[program_address_keypair.pubkey().as_ref()],
            &bpf_loader_upgradeable::id(),
        );

        // Load solana_bpf_rust_upgradeable program taken from solana test programs
        let program_data = read_governed_program("solana_bpf_rust_upgradeable");

        let program_buffer_rent = self
            .rent
            .minimum_balance(UpgradeableLoaderState::programdata_len(program_data.len()).unwrap());

        let mut instructions = bpf_loader_upgradeable::create_buffer(
            &self.payer.pubkey(),
            &program_buffer_keypair.pubkey(),
            &program_upgrade_authority_keypair.pubkey(),
            program_buffer_rent,
            program_data.len(),
        )
        .unwrap();

        let chunk_size = 800;

        for (chunk, i) in program_data.chunks(chunk_size).zip(0..) {
            instructions.push(bpf_loader_upgradeable::write(
                &program_buffer_keypair.pubkey(),
                &program_upgrade_authority_keypair.pubkey(),
                (i * chunk_size) as u32,
                chunk.to_vec(),
            ));
        }

        let program_account_rent = self
            .rent
            .minimum_balance(UpgradeableLoaderState::program_len().unwrap());

        let deploy_instructions = bpf_loader_upgradeable::deploy_with_max_program_len(
            &self.payer.pubkey(),
            &program_address_keypair.pubkey(),
            &program_buffer_keypair.pubkey(),
            &program_upgrade_authority_keypair.pubkey(),
            program_account_rent,
            program_data.len(),
        )
        .unwrap();

        instructions.extend_from_slice(&deploy_instructions);

        self.process_transaction(
            &instructions[..],
            Some(&[
                &program_upgrade_authority_keypair,
                &program_address_keypair,
                &program_buffer_keypair,
            ]),
        )
        .await;

        GovernedProgramSetup {
            address: program_address_keypair.pubkey(),
            upgrade_authority: program_upgrade_authority_keypair,
            data_address: program_data_address,
        }
    }

    pub async fn with_dummy_account(&mut self) {
        let instruction = create_dummy_account().unwrap();

        self.process_transaction(&[instruction], None).await;
    }

    pub async fn with_governance(
        &mut self,
        governed_program: &GovernedProgramSetup,
    ) -> GovernanceSetup {
        let (governance_address, _) = Pubkey::find_program_address(
            &[PROGRAM_AUTHORITY_SEED, governed_program.address.as_ref()],
            &id(),
        );

        let governance_mint = Pubkey::new_unique();
        let council_mint = Option::None::<Pubkey>;

        let vote_threshold: u8 = 60;
        let minimum_slot_waiting_period: u64 = 10;
        let time_limit: u64 = 100;
        let name = [0u8; GOVERNANCE_NAME_LENGTH];

        let create_governance_instruction = create_governance(
            &governance_address,
            &governed_program.address,
            &governed_program.data_address,
            &governed_program.upgrade_authority.pubkey(),
            &governance_mint,
            &self.payer.pubkey(),
            &council_mint,
            vote_threshold,
            minimum_slot_waiting_period,
            time_limit,
            &name,
        )
        .unwrap();

        self.process_transaction(
            &[create_governance_instruction],
            Some(&[&governed_program.upgrade_authority]),
        )
        .await;

        GovernanceSetup {
            address: governance_address,
            governance_mint,
            council_mint,
            vote_threshold,
            minimum_slot_waiting_period,
            time_limit,
            name,
        }
    }

    pub async fn get_governance_account(&mut self, governance_address: &Pubkey) -> Governance {
        let governance_account_raw = self
            .banks_client
            .get_account(*governance_address)
            .await
            .unwrap()
            .unwrap();

        Governance::unpack(&governance_account_raw.data).unwrap()
    }
}

fn get_governed_program_path(name: &str) -> PathBuf {
    let mut pathbuf = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    pathbuf.push("tests/program_test/programs");
    pathbuf.push(name);
    pathbuf.set_extension("_so");
    pathbuf
}

fn read_governed_program(name: &str) -> Vec<u8> {
    let path = get_governed_program_path(name);
    let mut file = File::open(&path).unwrap_or_else(|err| {
        panic!("Failed to open {}: {}", path.display(), err);
    });
    let mut elf = Vec::new();
    file.read_to_end(&mut elf).unwrap();

    elf
}
