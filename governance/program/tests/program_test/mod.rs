use borsh::BorshDeserialize;
use solana_program::{
    bpf_loader_upgradeable::{self, UpgradeableLoaderState},
    instruction::Instruction,
    program_pack::{IsInitialized, Pack},
    pubkey::Pubkey,
    rent::Rent,
    system_instruction,
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
    instruction::{
        create_governance, create_governance_realm, create_proposal, deposit_governing_tokens,
        withdraw_governing_tokens,
    },
    processor::process_instruction,
    state::{
        governance_realm::GovernanceRealm, program_governance::ProgramGovernance,
        proposal::Proposal, voter_record::VoterRecord,
    },
    tools::get_root_governance_address,
    PROGRAM_AUTHORITY_SEED,
};

pub mod cookies;
use self::cookies::{
    GovernanceRealmCookie, GovernedProgramCookie, ProgramGovernanceCookie, ProposalCookie,
    VoterRecordCookie,
};

pub mod programs;
use self::programs::read_test_program_elf;

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

    #[allow(dead_code)]
    pub async fn with_governed_program(&mut self) -> GovernedProgramCookie {
        let program_address_keypair = Keypair::new();
        let program_buffer_keypair = Keypair::new();
        let program_upgrade_authority_keypair = Keypair::new();

        let (program_data_address, _) = Pubkey::find_program_address(
            &[program_address_keypair.pubkey().as_ref()],
            &bpf_loader_upgradeable::id(),
        );

        // Load solana_bpf_rust_upgradeable program taken from solana test programs
        let program_data = read_test_program_elf("solana_bpf_rust_upgradeable");

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

        GovernedProgramCookie {
            address: program_address_keypair.pubkey(),
            upgrade_authority: program_upgrade_authority_keypair,
            data_address: program_data_address,
        }
    }

    #[allow(dead_code)]
    pub async fn with_dummy_governed_program(&mut self) -> GovernedProgramCookie {
        GovernedProgramCookie {
            address: Pubkey::new_unique(),
            upgrade_authority: Keypair::new(),
            data_address: Pubkey::new_unique(),
        }
    }

    #[allow(dead_code)]
    pub async fn with_program_governance(
        &mut self,
        governed_program: &GovernedProgramCookie,
    ) -> ProgramGovernanceCookie {
        let (governance_address, _) = Pubkey::find_program_address(
            &[PROGRAM_AUTHORITY_SEED, governed_program.address.as_ref()],
            &id(),
        );

        let governance_mint = Pubkey::new_unique();
        let council_mint = Option::None::<Pubkey>;

        let vote_threshold: u8 = 60;
        let min_instruction_hold_up_time: u64 = 10;
        let max_voting_time: u64 = 100;

        let create_governance_instruction = create_governance(
            &governance_address,
            &governed_program.address,
            &governed_program.data_address,
            &governed_program.upgrade_authority.pubkey(),
            &governance_mint,
            &self.payer.pubkey(),
            &council_mint,
            vote_threshold,
            min_instruction_hold_up_time,
            max_voting_time,
        )
        .unwrap();

        self.process_transaction(
            &[create_governance_instruction],
            Some(&[&governed_program.upgrade_authority]),
        )
        .await;

        ProgramGovernanceCookie {
            address: governance_address,
            governance_mint,
            council_mint,
            vote_threshold,
            min_instruction_hold_up_time,
            max_voting_time,
        }
    }

    #[allow(dead_code)]
    pub async fn get_program_governance_account(
        &mut self,
        governance_address: &Pubkey,
    ) -> ProgramGovernance {
        let governance_account_raw = self
            .banks_client
            .get_account(*governance_address)
            .await
            .unwrap()
            .unwrap();

        ProgramGovernance::unpack(&governance_account_raw.data).unwrap()
    }

    pub async fn get_account<T: BorshDeserialize>(&mut self, address: &Pubkey) -> T {
        let raw_account = self
            .banks_client
            .get_account(*address)
            .await
            .unwrap()
            .expect("Account missing");

        T::try_from_slice(&raw_account.data).unwrap()
    }

    #[allow(dead_code)]
    pub async fn with_proposal(&mut self, governance: &ProgramGovernanceCookie) -> ProposalCookie {
        let description_link = "proposal description".to_string();
        let name = "proposal_name".to_string();

        //let proposal_count = 0;
        let proposal_key = Keypair::new();

        let create_proposal_instruction = create_proposal(
            description_link.clone(),
            name.clone(),
            &proposal_key.pubkey(),
            &governance.address,
            &self.payer.pubkey(),
        )
        .unwrap();

        self.process_transaction(&[create_proposal_instruction], Some(&[&proposal_key]))
            .await;

        ProposalCookie {
            address: proposal_key.pubkey(),
            description_link: description_link,
            name: name,
        }
    }

    #[allow(dead_code)]
    pub async fn with_governance_realm(&mut self) -> GovernanceRealmCookie {
        let name = "Governance Realm".to_string();

        //let proposal_count = 0;
        let root_governance_key = get_root_governance_address(&name);

        let governance_token_mint_keypair = Keypair::new();
        let governance_token_mint_authority = Keypair::new();
        self.create_mint(
            &governance_token_mint_keypair,
            &governance_token_mint_authority.pubkey(),
        )
        .await;

        let governance_token_holding_keypair = Keypair::new();

        let council_mint_keypair = Keypair::new();
        let council_mint_authority = Keypair::new();
        self.create_mint(&council_mint_keypair, &council_mint_authority.pubkey())
            .await;

        let council_token_holding_keypair = Keypair::new();

        let create_proposal_instruction = create_governance_realm(
            name.clone(),
            &governance_token_mint_keypair.pubkey(),
            &governance_token_holding_keypair.pubkey(),
            &self.payer.pubkey(),
            Some(council_mint_keypair.pubkey()),
            Some(council_token_holding_keypair.pubkey()),
        )
        .unwrap();

        self.process_transaction(
            &[create_proposal_instruction],
            Some(&[
                &governance_token_holding_keypair,
                &council_token_holding_keypair,
            ]),
        )
        .await;

        GovernanceRealmCookie {
            address: root_governance_key,
            name,
            governance_mint: governance_token_mint_keypair.pubkey(),
            governance_mint_authority: governance_token_mint_authority,
            governance_token_holding_account: governance_token_holding_keypair.pubkey(),
            council_mint: Some(council_mint_keypair.pubkey()),
            council_token_holding_account: Some(council_token_holding_keypair.pubkey()),
            council_mint_authority: Some(council_mint_authority),
        }
    }

    #[allow(dead_code)]
    pub async fn with_initial_governance_token_deposit(
        &mut self,
        root_governance_setup: &GovernanceRealmCookie,
    ) -> VoterRecordCookie {
        let amount: u64 = 100;

        let voter_record_keypair = Keypair::new();
        let governance_token_source = Keypair::new();

        self.create_token_account(
            &governance_token_source,
            &root_governance_setup.governance_mint,
            &root_governance_setup.governance_mint_authority,
            amount + 100,
        )
        .await;

        let deposit_governing_tokens_instruction = deposit_governing_tokens(
            Some(amount),
            &root_governance_setup.address,
            &root_governance_setup.governance_mint,
            &root_governance_setup.governance_token_holding_account,
            &governance_token_source.pubkey(),
            &voter_record_keypair.pubkey(),
            &self.payer.pubkey(),
            true,
        )
        .unwrap();

        self.process_transaction(
            &[deposit_governing_tokens_instruction],
            Some(&[&voter_record_keypair]),
        )
        .await;

        VoterRecordCookie {
            address: voter_record_keypair.pubkey(),
            governance_token_deposit_amount: amount,

            governance_token_source: governance_token_source.pubkey(),
            council_token_deposit_amount: 0,

            council_token_source: None,
        }
    }

    #[allow(dead_code)]
    pub async fn with_governance_token_deposit(
        &mut self,
        root_governance_setup: &GovernanceRealmCookie,
        voter_record_setup: &VoterRecordCookie,
        amount: u64,
    ) {
        let deposit_governing_tokens_instruction = deposit_governing_tokens(
            Some(amount),
            &root_governance_setup.address,
            &root_governance_setup.governance_mint,
            &root_governance_setup.governance_token_holding_account,
            &voter_record_setup.governance_token_source,
            &voter_record_setup.address,
            &self.payer.pubkey(),
            false,
        )
        .unwrap();

        self.process_transaction(&[deposit_governing_tokens_instruction], None)
            .await;
    }

    #[allow(dead_code)]
    pub async fn withdraw_governance_token_deposit(
        &mut self,
        root_governance_setup: &GovernanceRealmCookie,
        voter_record_setup: &VoterRecordCookie,
        amount: u64,
    ) {
        let deposit_governing_tokens_instruction = withdraw_governing_tokens(
            Some(amount),
            &root_governance_setup.address,
            &root_governance_setup.governance_mint,
            &root_governance_setup.governance_token_holding_account,
            &voter_record_setup.governance_token_source,
            &voter_record_setup.address,
        )
        .unwrap();

        self.process_transaction(&[deposit_governing_tokens_instruction], None)
            .await;
    }

    #[allow(dead_code)]
    pub async fn with_council_token_deposit(
        &mut self,
        root_governance_setup: &GovernanceRealmCookie,
        voter_record_setup: &VoterRecordCookie,
        amount: u64,
    ) {
        let deposit_governing_tokens_instruction = deposit_governing_tokens(
            Some(amount),
            &root_governance_setup.address,
            &root_governance_setup.council_mint.unwrap(),
            &root_governance_setup.council_token_holding_account.unwrap(),
            &voter_record_setup.council_token_source.unwrap(),
            &voter_record_setup.address,
            &self.payer.pubkey(),
            false,
        )
        .unwrap();

        self.process_transaction(&[deposit_governing_tokens_instruction], None)
            .await;
    }

    #[allow(dead_code)]
    pub async fn with_initial_council_token_deposit(
        &mut self,
        root_governance_setup: &GovernanceRealmCookie,
    ) -> VoterRecordCookie {
        let amount: u64 = 10;

        let voter_record_keypair = Keypair::new();
        let council_token_source_account = Keypair::new();

        self.create_token_account(
            &council_token_source_account,
            &root_governance_setup.council_mint.unwrap(),
            &root_governance_setup
                .council_mint_authority
                .as_ref()
                .unwrap(),
            amount + 100,
        )
        .await;

        let deposit_governing_tokens_instruction = deposit_governing_tokens(
            Some(amount),
            &root_governance_setup.address,
            &root_governance_setup.council_mint.unwrap(),
            &root_governance_setup.council_token_holding_account.unwrap(),
            &council_token_source_account.pubkey(),
            &voter_record_keypair.pubkey(),
            &self.payer.pubkey(),
            true,
        )
        .unwrap();

        self.process_transaction(
            &[deposit_governing_tokens_instruction],
            Some(&[&voter_record_keypair]),
        )
        .await;

        VoterRecordCookie {
            address: voter_record_keypair.pubkey(),
            governance_token_deposit_amount: 0,
            governance_token_source: Pubkey::new_unique(),
            council_token_deposit_amount: amount,
            council_token_source: Some(council_token_source_account.pubkey()),
        }
    }

    #[allow(dead_code)]
    pub async fn get_voter_record_account(&mut self, address: &Pubkey) -> VoterRecord {
        self.get_account::<VoterRecord>(address).await
    }

    #[allow(dead_code)]
    pub async fn get_root_governnace_account(
        &mut self,
        root_governance_address: &Pubkey,
    ) -> GovernanceRealm {
        self.get_account::<GovernanceRealm>(root_governance_address)
            .await
    }

    #[allow(dead_code)]
    pub async fn get_proposal_account(&mut self, proposal_address: &Pubkey) -> Proposal {
        self.get_account::<Proposal>(proposal_address).await
    }

    #[allow(dead_code)]
    async fn get_packed_account<T: Pack + IsInitialized>(&mut self, address: &Pubkey) -> T {
        let raw_account = self
            .banks_client
            .get_account(*address)
            .await
            .unwrap()
            .unwrap();

        T::unpack(&raw_account.data).unwrap()
    }

    #[allow(dead_code)]
    pub async fn get_token_account(&mut self, address: &Pubkey) -> spl_token::state::Account {
        self.get_packed_account(address).await
    }

    pub async fn create_mint(&mut self, mint_keypair: &Keypair, mint_authority: &Pubkey) {
        let mint_rent = self.rent.minimum_balance(spl_token::state::Mint::LEN);

        let instructions = [
            system_instruction::create_account(
                &self.payer.pubkey(),
                &mint_keypair.pubkey(),
                mint_rent,
                spl_token::state::Mint::LEN as u64,
                &spl_token::id(),
            ),
            spl_token::instruction::initialize_mint(
                &spl_token::id(),
                &mint_keypair.pubkey(),
                &mint_authority,
                None,
                0,
            )
            .unwrap(),
        ];

        self.process_transaction(&instructions, Some(&[&mint_keypair]))
            .await;
    }

    pub async fn create_token_account(
        &mut self,
        token_account_keypair: &Keypair,
        token_mint: &Pubkey,
        token_mint_authority: &Keypair,
        amount: u64,
    ) {
        let create_account_instruction = system_instruction::create_account(
            &self.payer.pubkey(),
            &token_account_keypair.pubkey(),
            self.rent
                .minimum_balance(spl_token::state::Account::get_packed_len()),
            spl_token::state::Account::get_packed_len() as u64,
            &spl_token::id(),
        );

        let initialize_account_instruction = spl_token::instruction::initialize_account(
            &spl_token::id(),
            &token_account_keypair.pubkey(),
            token_mint,
            &self.payer.pubkey(),
        )
        .unwrap();

        let mint_instruction = spl_token::instruction::mint_to(
            &spl_token::id(),
            token_mint,
            &token_account_keypair.pubkey(),
            &token_mint_authority.pubkey(),
            &[],
            amount,
        )
        .unwrap();

        self.process_transaction(
            &[
                create_account_instruction,
                initialize_account_instruction,
                mint_instruction,
            ],
            Some(&[&token_account_keypair, &token_mint_authority]),
        )
        .await;
    }
}
