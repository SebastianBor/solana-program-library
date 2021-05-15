use borsh::BorshDeserialize;
use solana_program::{
    bpf_loader_upgradeable::{self, UpgradeableLoaderState},
    instruction::Instruction,
    program_error::ProgramError,
    program_pack::{IsInitialized, Pack},
    pubkey::Pubkey,
    rent::Rent,
    system_instruction,
};
use solana_program_test::ProgramTest;
use solana_program_test::*;

use solana_sdk::{
    signature::{Keypair, Signer},
    transaction::Transaction,
};
use spl_governance::{
    instruction::{
        create_account_governance, create_program_governance, create_proposal, create_realm,
        deposit_governing_tokens, set_vote_authority, withdraw_governing_tokens,
    },
    processor::process_instruction,
    state::{
        account_governance::{
            get_account_governance_address, get_program_governance_address, AccountGovernance,
        },
        enums::{GovernanceAccountType, GoverningTokenType, ProposalState},
        proposal::Proposal,
        realm::{get_governing_token_holding_address, get_realm_address, Realm},
        voter_record::{get_voter_record_address, VoterRecord},
    },
};

pub mod cookies;
use self::cookies::{
    AccountGovernanceCookie, GovernedAccountCookie, GovernedProgramCookie, ProposalCookie,
    RealmCookie, VoterRecordCookie,
};

pub mod tools;
use self::tools::{map_transaction_error, read_test_program_elf};

pub struct GovernanceProgramTest {
    pub banks_client: BanksClient,
    pub payer: Keypair,
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

        let (mut banks_client, payer, _) = program_test.start().await;

        let rent = banks_client.get_rent().await.unwrap();

        Self {
            banks_client,
            payer,
            rent,
        }
    }

    pub async fn process_transaction(
        &mut self,
        instructions: &[Instruction],
        signers: Option<&[&Keypair]>,
    ) -> Result<(), ProgramError> {
        let mut transaction =
            Transaction::new_with_payer(&instructions, Some(&self.payer.pubkey()));

        let mut all_signers = vec![&self.payer];

        if let Some(signers) = signers {
            all_signers.extend_from_slice(signers);
        }

        let recent_blockhash = self.banks_client.get_recent_blockhash().await.unwrap();

        transaction.sign(&all_signers, recent_blockhash);

        self.banks_client
            .process_transaction(transaction)
            .await
            .map_err(map_transaction_error)
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
        .await
        .unwrap();

        GovernedProgramCookie {
            address: program_address_keypair.pubkey(),
            upgrade_authority: program_upgrade_authority_keypair,
            data_address: program_data_address,
        }
    }

    #[allow(dead_code)]
    pub async fn with_governed_account(&mut self) -> GovernedAccountCookie {
        GovernedAccountCookie {
            address: Pubkey::new_unique(),
        }
    }

    #[allow(dead_code)]
    pub async fn with_account_governance(
        &mut self,
        realm_cookie: &RealmCookie,
        governed_account_cookie: &GovernedAccountCookie,
    ) -> AccountGovernanceCookie {
        let vote_threshold: u8 = 60;
        let min_instruction_hold_up_time: u64 = 10;
        let max_voting_time: u64 = 100;
        let token_threshold_to_create_proposal: u8 = 5;

        let create_account_governance_instruction = create_account_governance(
            &realm_cookie.address,
            &governed_account_cookie.address,
            vote_threshold,
            min_instruction_hold_up_time,
            max_voting_time,
            token_threshold_to_create_proposal,
            &self.payer.pubkey(),
        )
        .unwrap();

        let account = AccountGovernance {
            account_type: GovernanceAccountType::AccountGovernance,
            realm: realm_cookie.address,
            vote_threshold,
            token_threshold_to_create_proposal,
            min_instruction_hold_up_time,
            governed_account: governed_account_cookie.address,
            max_voting_time,
            proposal_count: 0,
        };

        self.process_transaction(&[create_account_governance_instruction], None)
            .await
            .unwrap();

        let account_governance_address =
            get_account_governance_address(&realm_cookie.address, &governed_account_cookie.address);

        AccountGovernanceCookie {
            address: account_governance_address,
            account,
        }
    }

    #[allow(dead_code)]
    pub async fn with_program_governance(
        &mut self,
        realm_cookie: &RealmCookie,
        governed_program_cookie: &GovernedProgramCookie,
    ) -> AccountGovernanceCookie {
        let vote_threshold: u8 = 60;
        let min_instruction_hold_up_time: u64 = 10;
        let max_voting_time: u64 = 100;
        let token_threshold_to_create_proposal: u8 = 5;

        let create_program_governance_instruction = create_program_governance(
            &realm_cookie.address,
            &governed_program_cookie.address,
            vote_threshold,
            min_instruction_hold_up_time,
            max_voting_time,
            token_threshold_to_create_proposal,
            &governed_program_cookie.data_address,
            &governed_program_cookie.upgrade_authority.pubkey(),
            &self.payer.pubkey(),
        )
        .unwrap();

        self.process_transaction(
            &[create_program_governance_instruction],
            Some(&[&governed_program_cookie.upgrade_authority]),
        )
        .await
        .unwrap();

        let account = AccountGovernance {
            account_type: GovernanceAccountType::AccountGovernance,
            realm: realm_cookie.address,
            vote_threshold,
            token_threshold_to_create_proposal,
            min_instruction_hold_up_time,
            governed_account: governed_program_cookie.address,
            max_voting_time,
            proposal_count: 0,
        };

        let program_governance_address =
            get_program_governance_address(&realm_cookie.address, &governed_program_cookie.address);

        AccountGovernanceCookie {
            address: program_governance_address,
            account,
        }
    }

    pub async fn get_account<T: BorshDeserialize>(&mut self, address: &Pubkey) -> T {
        let raw_account = self
            .banks_client
            .get_account(*address)
            .await
            .unwrap()
            .expect("GET TEST ACCOUNT: Account not found");

        T::try_from_slice(&raw_account.data).unwrap()
    }

    #[allow(dead_code)]
    pub async fn with_community_proposal(
        &mut self,
        account_governance_cookie: &AccountGovernanceCookie,
    ) -> ProposalCookie {
        self.with_proposal(account_governance_cookie, GoverningTokenType::Governance)
            .await
    }

    #[allow(dead_code)]
    pub async fn with_proposal(
        &mut self,
        account_governance_cookie: &AccountGovernanceCookie,
        governing_token_type: GoverningTokenType,
    ) -> ProposalCookie {
        let description_link = "Proposal Description".to_string();
        let name = "Proposal Name".to_string();

        let proposal_keypair = Keypair::new();

        let create_proposal_instruction = create_proposal(
            name.clone(),
            governing_token_type.clone(),
            description_link.clone(),
            &proposal_keypair.pubkey(),
            &account_governance_cookie.address,
            &self.payer.pubkey(),
        )
        .unwrap();

        self.process_transaction(&[create_proposal_instruction], Some(&[&proposal_keypair]))
            .await
            .unwrap();

        let account = Proposal {
            account_type: GovernanceAccountType::Proposal,
            description_link,
            name,
            account_governance: account_governance_cookie.address,
            governing_token_type,
            state: ProposalState::Draft,
        };

        ProposalCookie {
            address: proposal_keypair.pubkey(),
            account,
        }
    }

    #[allow(dead_code)]
    pub async fn with_realm(&mut self) -> RealmCookie {
        let name = "Realm".to_string();

        let realm_address = get_realm_address(&name);

        let governance_token_mint_keypair = Keypair::new();
        let governance_token_mint_authority = Keypair::new();

        let governance_token_holding_address = get_governing_token_holding_address(
            &realm_address,
            &governance_token_mint_keypair.pubkey(),
        );

        self.create_mint(
            &governance_token_mint_keypair,
            &governance_token_mint_authority.pubkey(),
        )
        .await;

        let council_token_mint_keypair = Keypair::new();
        let council_token_mint_authority = Keypair::new();

        let council_token_holding_address = get_governing_token_holding_address(
            &realm_address,
            &council_token_mint_keypair.pubkey(),
        );

        self.create_mint(
            &council_token_mint_keypair,
            &council_token_mint_authority.pubkey(),
        )
        .await;

        let create_proposal_instruction = create_realm(
            name.clone(),
            &governance_token_mint_keypair.pubkey(),
            &self.payer.pubkey(),
            Some(council_token_mint_keypair.pubkey()),
        )
        .unwrap();

        self.process_transaction(&[create_proposal_instruction], None)
            .await
            .unwrap();

        let account = Realm {
            account_type: GovernanceAccountType::Realm,
            governance_mint: governance_token_mint_keypair.pubkey(),
            council_mint: Some(council_token_mint_keypair.pubkey()),
            name: name,
        };

        RealmCookie {
            address: realm_address,
            account,

            governance_mint_authority: governance_token_mint_authority,
            governance_token_holding_account: governance_token_holding_address,

            council_token_holding_account: Some(council_token_holding_address),
            council_mint_authority: Some(council_token_mint_authority),
        }
    }

    #[allow(dead_code)]
    pub async fn with_initial_governance_token_deposit(
        &mut self,
        realm_cookie: &RealmCookie,
    ) -> VoterRecordCookie {
        self.with_initial_governaning_token_deposit(
            &realm_cookie.address,
            GoverningTokenType::Governance,
            &realm_cookie.account.governance_mint,
            &realm_cookie.governance_mint_authority,
        )
        .await
    }

    #[allow(dead_code)]
    pub async fn with_governance_token_deposit(
        &mut self,
        realm_cookie: &RealmCookie,
        voter_record_cookie: &VoterRecordCookie,
        amount: u64,
    ) {
        self.with_governing_token_deposit(
            &realm_cookie.address,
            &realm_cookie.account.governance_mint,
            &realm_cookie.governance_mint_authority,
            voter_record_cookie,
            amount,
        )
        .await;
    }

    #[allow(dead_code)]
    pub async fn with_council_token_deposit(
        &mut self,
        realm_cookie: &RealmCookie,
        voter_record_cookie: &VoterRecordCookie,
        amount: u64,
    ) {
        self.with_governing_token_deposit(
            &realm_cookie.address,
            &realm_cookie.account.council_mint.unwrap(),
            &realm_cookie.council_mint_authority.as_ref().unwrap(),
            voter_record_cookie,
            amount,
        )
        .await;
    }

    #[allow(dead_code)]
    pub async fn with_initial_council_token_deposit(
        &mut self,
        realm_cookie: &RealmCookie,
    ) -> VoterRecordCookie {
        self.with_initial_governaning_token_deposit(
            &realm_cookie.address,
            GoverningTokenType::Council,
            &realm_cookie.account.council_mint.unwrap(),
            &realm_cookie.council_mint_authority.as_ref().unwrap(),
        )
        .await
    }

    #[allow(dead_code)]
    pub async fn with_initial_governaning_token_deposit(
        &mut self,
        realm_address: &Pubkey,
        governing_token_type: GoverningTokenType,
        governing_mint: &Pubkey,
        governing_mint_authority: &Keypair,
    ) -> VoterRecordCookie {
        let token_owner = Keypair::new();
        let token_source = Keypair::new();

        let source_amount = 100;
        let vote_authority = Keypair::new();

        self.create_token_account(
            &token_source,
            governing_mint,
            governing_mint_authority,
            source_amount,
            token_owner.pubkey(),
        )
        .await;

        let deposit_governing_tokens_instruction = deposit_governing_tokens(
            realm_address,
            governing_mint,
            &token_source.pubkey(),
            &token_owner.pubkey(),
            &self.payer.pubkey(),
        )
        .unwrap();

        self.process_transaction(
            &[deposit_governing_tokens_instruction],
            Some(&[&token_owner]),
        )
        .await
        .unwrap();

        let voter_record_address =
            get_voter_record_address(realm_address, &governing_mint, &token_owner.pubkey());

        let account = VoterRecord {
            account_type: GovernanceAccountType::VoterRecord,
            realm: *realm_address,
            token_type: governing_token_type,
            token_owner: token_owner.pubkey(),
            token_deposit_amount: source_amount,
            vote_authority: token_owner.pubkey(),
            active_votes_count: 0,
            total_votes_count: 0,
        };

        VoterRecordCookie {
            address: voter_record_address,
            account,

            token_source_amount: source_amount,
            token_source: token_source.pubkey(),
            token_owner,
            vote_authority,
        }
    }

    #[allow(dead_code)]
    async fn with_governing_token_deposit(
        &mut self,
        realm: &Pubkey,
        governing_token_mint: &Pubkey,
        governing_token_mint_authority: &Keypair,
        voter_record_cookie: &VoterRecordCookie,
        amount: u64,
    ) {
        self.mint_tokens(
            governing_token_mint,
            governing_token_mint_authority,
            &voter_record_cookie.token_source,
            amount,
        )
        .await;

        let deposit_governing_tokens_instruction = deposit_governing_tokens(
            realm,
            governing_token_mint,
            &voter_record_cookie.token_source,
            &voter_record_cookie.token_owner.pubkey(),
            &self.payer.pubkey(),
        )
        .unwrap();

        self.process_transaction(
            &[deposit_governing_tokens_instruction],
            Some(&[&voter_record_cookie.token_owner]),
        )
        .await
        .unwrap();
    }

    #[allow(dead_code)]
    pub async fn with_governance_vote_authority(
        &mut self,
        realm_cookie: &RealmCookie,
        voter_record_cookie: &VoterRecordCookie,
    ) {
        self.with_governing_token_vote_authority(
            &realm_cookie.address,
            &realm_cookie.account.governance_mint,
            &voter_record_cookie,
        )
        .await;
    }

    #[allow(dead_code)]
    pub async fn with_council_vote_authority(
        &mut self,
        realm_cookie: &RealmCookie,
        voter_record_cookie: &VoterRecordCookie,
    ) {
        self.with_governing_token_vote_authority(
            &realm_cookie.address,
            &realm_cookie.account.council_mint.unwrap(),
            &voter_record_cookie,
        )
        .await;
    }

    #[allow(dead_code)]
    pub async fn with_governing_token_vote_authority(
        &mut self,
        realm: &Pubkey,
        governing_token_mint: &Pubkey,
        voter_record_cookie: &VoterRecordCookie,
    ) {
        let set_vote_authority_instruction = set_vote_authority(
            realm,
            governing_token_mint,
            &voter_record_cookie.vote_authority.pubkey(),
            &voter_record_cookie.token_owner.pubkey(),
        )
        .unwrap();

        self.process_transaction(
            &[set_vote_authority_instruction],
            Some(&[&voter_record_cookie.token_owner]),
        )
        .await
        .unwrap();
    }

    #[allow(dead_code)]
    pub async fn withdraw_governance_tokens(
        &mut self,
        realm_cookie: &RealmCookie,
        voter_record_cookie: &VoterRecordCookie,
    ) -> Result<(), ProgramError> {
        self.withdraw_governing_tokens(
            realm_cookie,
            voter_record_cookie,
            &realm_cookie.account.governance_mint,
            &voter_record_cookie.token_owner,
        )
        .await
    }

    #[allow(dead_code)]
    pub async fn withdraw_council_tokens(
        &mut self,
        realm_cookie: &RealmCookie,
        voter_record_cookie: &VoterRecordCookie,
    ) -> Result<(), ProgramError> {
        self.withdraw_governing_tokens(
            realm_cookie,
            voter_record_cookie,
            &realm_cookie.account.council_mint.unwrap(),
            &voter_record_cookie.token_owner,
        )
        .await
    }

    #[allow(dead_code)]
    async fn withdraw_governing_tokens(
        &mut self,
        realm_cookie: &RealmCookie,
        voter_record_cookie: &VoterRecordCookie,
        governing_token_mint: &Pubkey,

        governing_token_owner: &Keypair,
    ) -> Result<(), ProgramError> {
        let deposit_governing_tokens_instruction = withdraw_governing_tokens(
            &realm_cookie.address,
            governing_token_mint,
            &voter_record_cookie.token_source,
            &governing_token_owner.pubkey(),
        )
        .unwrap();

        self.process_transaction(
            &[deposit_governing_tokens_instruction],
            Some(&[&governing_token_owner]),
        )
        .await
    }

    #[allow(dead_code)]
    pub async fn get_program_governance_account(
        &mut self,
        program_governance_address: &Pubkey,
    ) -> AccountGovernance {
        self.get_account::<AccountGovernance>(program_governance_address)
            .await
    }

    #[allow(dead_code)]
    pub async fn get_voter_record_account(&mut self, address: &Pubkey) -> VoterRecord {
        self.get_account::<VoterRecord>(address).await
    }

    #[allow(dead_code)]
    pub async fn get_realm_account(&mut self, root_governance_address: &Pubkey) -> Realm {
        self.get_account::<Realm>(root_governance_address).await
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
            .await
            .unwrap();
    }

    pub async fn create_token_account(
        &mut self,
        token_account_keypair: &Keypair,
        token_mint: &Pubkey,
        token_mint_authority: &Keypair,
        amount: u64,
        owner: Pubkey,
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
            &owner,
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
        .await
        .unwrap();
    }

    pub async fn mint_tokens(
        &mut self,
        token_mint: &Pubkey,
        token_mint_authority: &Keypair,
        token_account: &Pubkey,
        amount: u64,
    ) {
        let mint_instruction = spl_token::instruction::mint_to(
            &spl_token::id(),
            &token_mint,
            &token_account,
            &token_mint_authority.pubkey(),
            &[],
            amount,
        )
        .unwrap();

        self.process_transaction(&[mint_instruction], Some(&[&token_mint_authority]))
            .await
            .unwrap();
    }
}
