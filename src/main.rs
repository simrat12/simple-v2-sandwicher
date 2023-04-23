use web3::types::{U256, H256, BlockNumber};
use web3::types::transaction::{Transaction, RawTransaction};
use web3::transports::WebSocket;
use web3::ethabi::{Token, Address};
use web3::api::Eth;
use web3::Web3;
use ethers::prelude::*;
use std::env;
use dotenv::dotenv;

use tokio::runtime::Runtime;
use tokio::task;

use serde::{Serialize, Deserialize};

use anyhow::{Context, Result};

use once_cell::sync::Lazy;
use thread_utils::sandwich_threads::max_sandwich_constraints;

const WEBSOCKET_URL: &str = "ws://localhost:8545";
const FLASHBOTS_RELAY_URL: &str = "https://relay.flashbots.net/";

const LOWER_BOUND_PROFIT: u64 = 0;
const UPPER_BOUND_SAND: u64 = 0.25 * 10u64.pow(18);

const BUNDLE_FILE: &str = "dump/bundle.txt";

const HOOK_CODE: &str = include_str!("../build/contracts/ShinySporkProject.json");
const SANDWICH_CONTRACT_ADDRESS: &str = "***REMOVED***";

static WEB3: Lazy<Web3<WebSocket>> = Lazy::new(|| Web3::new(WebSocket::new(WEBSOCKET_URL).unwrap()));

//need to import GlobalContracts

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PendingTransaction {
    from: Address,
    to: Address,
    value: U256,
    gas: U256,
    nonce: U256,
    hash: H256,
    r: H256,
    s: H256,
    raw: Vec<u8>
}

use std::fs::File;
use std::io::BufReader;
use std::path::Path;

use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
struct ContractDictionary {
    uni_router: Contract,
    sushi_router: Contract,
    inch_router: Contract,
    v3_router: Contract,
}


async fn start_ganache_fork(block_number: u64) -> Result<Child, Box<dyn std::error::Error>> {
    let infura_project_id = "YOUR-INFURA-PROJECT-ID";
    let fork_url = format!("https://mainnet.infura.io/v3/{}@{}", infura_project_id, block_number);

    let ganache_process = Command::new("ganache-cli")
        .arg("--fork")
        .arg(&fork_url)
        .spawn()?;

    println!("Ganache CLI started with fork at block: {}", block_number);

    // Give Ganache some time to start up before connecting with your Rust code
    tokio::time::sleep(Duration::from_secs(5)).await;

    Ok(ganache_process)
}

async fn get_pending_transactions(
    provider: &web3,
    block_provider: &web3,
    raw_pending_transactions: Vec<Transaction>,
    current_block_number: u64,
    real_priority_fee: U256,
) -> Result<(), Box<dyn Error>> {
    let to_list = vec![
        global_contracts.uni_router.address.to_lowercase(),
        global_contracts.sushi_router.address.to_lowercase(),
        global_contracts.inch_router.address.to_lowercase(),
        global_contracts.v3_router.address.to_lowercase(),
    ];
    

    let pending_transactions: Vec<Transaction> = raw_pending_transactions
        .into_iter()
        .filter(|tx| {
            tx.to
                .as_ref()
                .map(|to| to_list.contains(&to.to_string().to_lowercase()))
                .unwrap_or(false)
        })
        .collect();

    let swap_dict = thread_initialize_class(
        provider.clone(),
        &global_contracts,
        &pending_transactions,
        UPPER_BOUND_SAND,
        ).await;

    let mainnet_flashbots = env::var("MAINNET_FLASHBOTS").expect("MAINNET_FLASHBOTS not set");
    let executor_wallet = env::var("MAINNET_WALLET").expect("MAINNET_WALLET not set");

    for tx in pending_transactions {
        let sandwich_tx_candidate = max_sandwich_constraints(   
            &swap_dict,
            LOWER_BOUND_PROFIT,
            UPPER_BOUND_SAND,
        );

        let sandwich = Sandwich::new(
            &block_provider,
            &account,
            &flashbots_account,
            &sandwich_contract,
            &swap_transaction,
            false,
            None,
        );

        match sandwich.make_sandwich(current_block_number, real_priority_fee).await {
            Ok((bundle, swap_hash, real_priority_fee, bundle_hash)) => {
                // Create flashbots client
                let flashbots_client =
                    FlashbotsClient::new(provider.clone(), mainnet_flashbots, None);

                // Submit bundle
                let submission_result = flashbots_client
                    .send_bundle(bundle, swap_hash, real_priority_fee)
                    .await;

                // Handle bundle submission result
                match submission_result {
                    Ok(bundle_submission) => {
                        println!("Bundle submitted! {:?}", bundle_submission);

                        // Change provider to the mainnet provider
                        let mainnet_provider =
                            Provider::<Http>::try_from("https://mainnet.infura.io/v3/YOUR_API_KEY")?;

                        // Send the bundle to the mainnet provider
                        let send_result = mainnet_provider
                            .flashbots()
                            .send_bundle(bundle, target_block_number)
                            .await?;

                        println!("Bundle sent to mainnet: {:?}", send_result);
                    }
                    Err(submission_err) => {
                        println!("Bundle submission error: {:?}", submission_err);
                    }
                }
            }
            Err(err) => {
                println!("Error while making sandwich: {:?}", err);
            }
        }
    }
    Ok(())
}

fn main() -> Result<(), Box<dyn Error>> {
    let rt = Runtime::new().unwrap();

    let block_provider = web3::Web3::new(web3::transports::WebSocket::new("ws://localhost:8545").unwrap());
    let provider = web3::Web3::new(web3::transports::WebSocket::new("<INFURA_WS_API_URL>").unwrap());
    let http = Http::new("https://mainnet.infura.io/v3/YOUR-PROJECT-ID").unwrap();  //not sure which one to feed into globalContracts
    let web3 = Web3::new(http);
    let eth = web3.eth();
    let path = "external_contracts/v2_contracts.dictionary".to_string();

    // Initialize the GlobalContracts struct
    let global_contracts = match GlobalContracts::new(eth, path) {
        Ok(contracts) => contracts,
        Err(e) => {
            eprintln!("Error initializing GlobalContracts: {}", e);
            return;
        }
    };
    let pending_transactions: HashMap<H256, Transaction> = HashMap::new();
    let mut ignore_transactions: HashMap<H256, Transaction> = HashMap::new();

    // create and start timer to clear ignore_transactions
    let mut clear_timer = tokio::time::interval(Duration::from_secs(300));
    task::spawn(async move {
        loop {
            clear_timer.tick().await;
            ignore_transactions.clear();
        }
    });

    rt.block_on(async move {
        // Load contract ABI
        let hook_code = serde_json::from_str(HOOK_CODE).unwrap();
        let abi = hook_code.abi;
        let sandwich_contract = Contract::new(provider.eth(), Address::from_slice(SANDWICH_CONTRACT_ADDRESS.as_bytes()), abi);

        // Create bundle file and lock
        let bundle_lock = Arc::new(Mutex::new(()));
        let bundle_file = std::fs::OpenOptions::new().read(true).write(true).create(true).open(BUNDLE_FILE)?;
        let bundle_file = Arc::new(Mutex::new(bundle_file));

        // Get current block number
        let current_block_number = provider.eth().block_number().await.unwrap().as_u64();

        // Start a local Ganache fork at the most recent block number
        let mut ganache_process = start_ganache_fork(current_block_number).await?;

        // Create a pending transactions filter
        let filter = provider.eth_filter().create_pending_transactions_filter().await?;

        // Get the new pending transaction hashes
        let new_transaction_hashes = filter.get_all_changes().await?;

        // Fetch the pending transaction data for each hash
        let raw_pending_transactions: Vec<Transaction> = new_transaction_hashes
            .into_iter()
            .filter_map(|tx_hash| provider.eth().transaction(tx_hash).await.unwrap_or(None))
            .collect();

        // Probably need to define provider as a web3 instance only
        let pending_transactions = get_pending_transactions(&block_provider, raw_pending_transactions, current_block_number, real_priority_fee).await?; // need to pass in correct arguments here

        Ok(())
    }).unwrap();

    Ok(())
}

