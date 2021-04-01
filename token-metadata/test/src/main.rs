use {
    clap::{crate_description, crate_name, crate_version, App, Arg},
    solana_clap_utils::input_validators::{is_url, is_valid_signer},
    solana_client::rpc_client::RpcClient,
    solana_program::{borsh::try_from_slice_unchecked, program_pack::Pack},
    solana_sdk::{
        pubkey::Pubkey,
        signature::{read_keypair_file, Signer},
        system_instruction::create_account,
        transaction::Transaction,
    },
    spl_token::{instruction::initialize_mint, state::Mint},
    spl_token_metadata::{
        instruction::{create_metadata_accounts, update_metadata_accounts},
        state::{Metadata, PREFIX},
    },
    std::str::FromStr,
};

const METADATA_PROGRAM_PUBKEY: &str = "meta75ZHbozdG3sYzM6PdN7PNK6w9PgsAEEjVYKoAKr";
const TOKEN_PROGRAM_PUBKEY: &str = "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA";

fn main() -> () {
    let app_matches = App::new(crate_name!())
        .about(crate_description!())
        .version(crate_version!())
        .arg(
            Arg::with_name("keypair")
                .long("keypair")
                .value_name("KEYPAIR")
                .required(true)
                .validator(is_valid_signer)
                .takes_value(true)
                .help("Filepath or URL to a keypair"),
        )
        .arg(
            Arg::with_name("mint_keypair")
                .long("mint_keypair")
                .required(true)
                .value_name("MINT_KEYPAIR")
                .validator(is_valid_signer)
                .takes_value(true)
                .help("Filepath or URL to a keypair"),
        )
        .arg(
            Arg::with_name("json_rpc_url")
                .long("url")
                .value_name("URL")
                .takes_value(true)
                .global(true)
                .validator(is_url)
                .help("JSON RPC URL for the cluster [default: devnet]"),
        )
        .arg(
            Arg::with_name("name")
                .long("name")
                .required(true)
                .value_name("NAME")
                .takes_value(true)
                .help("name for the Mint"),
        )
        .arg(
            Arg::with_name("symbol")
                .long("symbol")
                .value_name("SYMBOL")
                .takes_value(true)
                .required(true)
                .help("symbol for the Mint"),
        )
        .arg(
            Arg::with_name("uri")
                .long("uri")
                .value_name("URI")
                .takes_value(true)
                .required(true)
                .help("URI for the Mint"),
        )
        .arg(
            Arg::with_name("update_uri")
                .long("update_uri")
                .value_name("UPDATE_URI")
                .takes_value(true)
                .required(true)
                .help("URI for the Mint to be updated with after creation to test update call"),
        )
        .get_matches();

    let client = RpcClient::new(
        app_matches
            .value_of("json_rpc_url")
            .unwrap_or(&"https://devnet.solana.com".to_owned())
            .to_owned(),
    );
    let payer = read_keypair_file(app_matches.value_of("keypair").unwrap()).unwrap();
    let program_key = Pubkey::from_str(METADATA_PROGRAM_PUBKEY).unwrap();
    let token_key = Pubkey::from_str(TOKEN_PROGRAM_PUBKEY).unwrap();
    let new_mint = read_keypair_file(app_matches.value_of("mint_keypair").unwrap()).unwrap();
    let name = app_matches.value_of("name").unwrap().to_owned();
    let symbol = app_matches.value_of("symbol").unwrap().to_owned();
    let uri = app_matches.value_of("uri").unwrap().to_owned();
    let update_uri = app_matches.value_of("update_uri").unwrap().to_owned();
    let new_mint_key = new_mint.pubkey();
    let metadata_seeds = &[
        PREFIX.as_bytes(),
        &program_key.as_ref(),
        new_mint_key.as_ref(),
    ];
    let (metadata_key, _) = Pubkey::find_program_address(metadata_seeds, &program_key);

    let owner_seeds = &[
        PREFIX.as_bytes(),
        &program_key.as_ref(),
        &name.as_bytes(),
        &symbol.as_bytes(),
    ];
    let (owner_key, _) = Pubkey::find_program_address(owner_seeds, &program_key);

    let mut transaction = Transaction::new_with_payer(
        &[
            create_account(
                &payer.pubkey(),
                &new_mint.pubkey(),
                client
                    .get_minimum_balance_for_rent_exemption(Mint::LEN)
                    .unwrap(),
                Mint::LEN as u64,
                &token_key,
            ),
            initialize_mint(
                &token_key,
                &new_mint.pubkey(),
                &payer.pubkey(),
                Some(&payer.pubkey()),
                0,
            )
            .unwrap(),
            create_metadata_accounts(
                program_key,
                owner_key,
                metadata_key,
                new_mint.pubkey(),
                payer.pubkey(),
                payer.pubkey(),
                payer.pubkey(),
                name,
                symbol,
                uri,
            ),
            update_metadata_accounts(
                program_key,
                metadata_key,
                owner_key,
                payer.pubkey(),
                update_uri.to_owned(),
            ),
        ],
        Some(&payer.pubkey()),
    );
    let recent_blockhash = client.get_recent_blockhash().unwrap().0;
    transaction.sign(&[&payer, &new_mint], recent_blockhash);
    client.send_and_confirm_transaction(&transaction).unwrap();
    let account = client.get_account(&metadata_key).unwrap();
    let metadata: Metadata = try_from_slice_unchecked(&account.data).unwrap();
    println!(
        "If this worked correctly, updated metadata should have {:?}: {:?} ",
        update_uri, metadata.uri
    );
}