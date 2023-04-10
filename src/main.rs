use web3::types::{U256, H256, BlockNumber};
use web3::types::transaction::{Transaction, RawTransaction};
use web3::transports::WebSocket;
use web3::ethabi::{Token, Address};
use web3::api::Eth;
use web3::Web3;

use tokio::runtime::Runtime;
use tokio::task;

use serde::{Serialize, Deserialize};

use anyhow::{Context, Result};

use once_cell::sync::Lazy;

const WEBSOCKET_URL: &str = "ws://localhost:8545";
const FLASHBOTS_RELAY_URL: &str = "https://relay.flashbots.net/";

const LOWER_BOUND_PROFIT: u64 = 0;
const UPPER_BOUND_SAND: u64 = 0.25 * 10u64.pow(18);

const BUNDLE_FILE: &str = "dump/bundle.txt";

const HOOK_CODE: &str = include_str!("../build/contracts/ShinySporkProject.json");
const SANDWICH_CONTRACT_ADDRESS: &str = "***REMOVED***";

static WEB3: Lazy<Web3<WebSocket>> = Lazy::new(|| Web3::new(WebSocket::new(WEBSOCKET_URL).unwrap()));

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

#[derive(Debug, Serialize, Deserialize)]
struct Contract {
    address: String,
}

struct GlobalContracts {
    uni_router: Contract,
    sushi_router: Contract,
    inch_router: Contract,
    v3_router: Contract,
}

impl GlobalContracts {
    fn new(block_provider: web3::Web3<web3::transports::WebSocket>) -> Self {
        let path = Path::new("external_contracts/v2_contracts.dictionary");
        let file = File::open(path).expect("Unable to open contract dictionary file");
        let reader = BufReader::new(file);
        let dictionary: ContractDictionary = serde_json::from_reader(reader).expect("Unable to deserialize contract dictionary");

        GlobalContracts {
            uni_router: dictionary.uni_router,
            sushi_router: dictionary.sushi_router,
            inch_router: dictionary.inch_router,
            v3_router: dictionary.v3_router,
        }
    }
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

async fn get_pending_transactions(provider: &Web3<Http>, new_transactions: Vec<H256>) -> HashMap<H256, Transaction> {
    let mut pending_transactions = HashMap::new();

    for entry in new_transactions {
        match provider.eth().transaction(entry).await {
            Ok(Some(tx)) => {
                let tx_to = tx.to;
                let tx_gas = tx.gas.unwrap().as_u64();
                let tx_value = tx.value.unwrap().as_u64();
                let tx_nonce = tx.nonce.as_u64();
                let tx_from = tx.from.unwrap();

                if TO_LIST.iter().any(|&i| i == tx_to)
                    && tx_gas > 80000
                    && tx_value > 0.1 * 10u64.pow(18)
                    && tx_nonce >= provider.eth().transaction_count(tx_from, None).await.unwrap().as_u64()
                {
                    pending_transactions.insert(tx.hash, tx);
                }
            }
            _ => (),
        }
    }

    pending_transactions
}

fn main() {
    let block_provider = web3::Web3::new(web3::transports::WebSocket::new("ws://localhost:8545").unwrap());
    let global_contracts = GlobalContracts::new(block_provider);
    let TO_LIST = vec![
        global_contracts.uni_router.address.to_lowercase(),
        global_contracts.sushi_router.address.to_lowercase(),
        global_contracts.inch_router.address.to_lowercase(),
        global_contracts.v3_router.address.to_lowercase(),
    ];
    let pending_transactions: HashMap<H256, Transaction> = HashMap::new();
    let rt = Runtime::new().unwrap();
    let mut ignore_transactions: HashMap<H256, Transaction> = HashMap::new();

    // create and start timer to clear ignore_transactions
    let mut clear_timer = tokio::time::interval(Duration::from_secs(300));
    task::spawn(async move {
        loop {
            clear_timer.tick().await;
            ignore_transactions.clear();
        }
    });

    let provider = WEB3.clone();
    let transport = provider.transport();
    let task_handle = task::spawn(async move {
        let mut current_block = provider.eth().block_number().await.expect("Unable to get current block");
        loop {
            let latest_block = provider.eth().block_number().await.expect("Unable to get latest block");
            if latest_block > current_block {
                println!("New block found at {}, forking...", latest_block);
                transport.fork(BlockId::Number(BlockNumber::Number(latest_block.as_u64())));
                current_block = latest_block;
            }
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
    
        // Start pending transactions filter
        let filter = provider.eth_filter().create_pending().await?;
        let new_transactions = filter.get_changes().await?;
        let pending_transactions = get_pending_transactions(provider.clone(), new_transactions).await?;
        let filter_id = filter.id;
        let mut filter_stream = filter.stream(Duration::from_secs(60));
        let mut ganache_process = start_ganache_fork(current_block_number).await?;
        //NEED TO DEFINE TX VIA SENDING 
        let swap_dict = thread_initialize_class(block_provider.clone(), global_contracts.clone(), pending_transactions.clone(), upper_bound_sand).await;

        if let Some(swap_dict) = swap_dict {
            let sandwich_tx_candidate = max_sandwich_constraints(swap_dict, lower_bound_profits, upper_bound_sand);
    
        // Process pending transactions
        //can likely remove the below while statement, and just check if the sandwich works with the forked local goerli node
        while let Some(tx) = filter_stream.next().await {
            match tx {
                Ok(tx) => {
                    let tx_hash = tx.hash;
                    let tx = provider.eth().transaction(tx_hash).await.unwrap().unwrap();
                    let tx_to = tx.to;
                    let tx_gas = tx.gas.unwrap().as_u64();
                    let tx_value = tx.value.unwrap().as_u64();
                    let tx_nonce = tx.nonce.as_u64();
                    let tx_from = tx.from.unwrap();
                    let tx_raw = tx.raw.unwrap();
    
                    // Check if transaction is relevant
                    if TO_LIST.iter().any(|&i| i == tx_to) &&
                        tx_gas > 80000 &&
                        tx_value > 0.1 * 10u64.pow(18) &&
                        tx_nonce >= provider.eth().transaction_count(tx_from, None).await.unwrap().as_u64()
                    {
                        // Check if transaction should be ignored
                        let ignore_txs = ignore_transactions.lock().unwrap();
                        if ignore_txs.contains_key(&tx_hash) {
                            continue;
                        }
    
                        // Get current block number
                        let current_block_number = provider.eth().block_number().await.unwrap().as_u64();
    
                        // Initialize sandwich
                        let sandwich = Sandwich::new(provider.clone(), sandwich_contract.clone(), tx.clone(), bundle_lock.clone(), bundle_file.clone(), lower_bound_profits, upper_bound_sand);
    
                        // Check if sandwich can be made
                        match sandwich.make_sandwich(current_block_number, real_priority_fee).await {
                            Ok((bundle, swap_hash, real_priority_fee, bundle_hash)) => {
                                // Create flashbots client
                                let flashbots_client = FlashbotsClient::new(provider.clone(), flashbots_account.clone(), None);
    
                                // Submit bundle
                                let submission_result = flashbots_client.send_bundle(bundle, swap_hash, real_priority_fee).await;
    
                                // Handle bundle submission result
                                match submission_result {
                                    Ok(bundle_submission) => {
                                        println!("Bundle submitted! {:?}", bundle_submission);
    
                                        // Reset fork
                                        let reset_result = reset_fork().await;
                                        if reset_result.is_err() {
                                            return Err(format!("Failed to reset fork: {:?}", reset_result.unwrap_err()));
                                        }
    
                                        // Reset filter
                                        provider.eth_filter().uninstall(filter_id).await?;
                                        filter_stream = provider.eth_filter().create_pending().await?.stream(Duration::from_secs(60));
                                    },
                                    Err(submission_err) => {
                                        // Handle submission error
                                        println!("Bundle submission error: {:?}", submission_err);
    
                                        // Ignore transaction for specified period of time
                                        let mut ignore_txs = ignore_transactions.lock().unwrap();
                                        if !ignore_txs.contains_key(&tx_hash) {
                                            ignore_txs.insert(tx_hash, Instant::now());
                                        }
                                        println!("Bundle submission error: {:?}", submission_err);
    
                                        // Ignore transaction for specified period of time
                                        let mut ignore_txs = ignore_transactions.lock().unwrap();
                                        if !ignore_txs.contains_key(&tx_hash) {
                                            ignore_txs.insert(tx_hash, Instant::now());
                                        }
                                        drop(ignore_txs);
    
                                        // Wait until ignore period is over
                                        loop {
                                            // Check if ignore period is over
                                            let mut ignore_txs = ignore_transactions.lock().unwrap();
                                            if ignore_txs.contains_key(&tx_hash) && ignore_txs[&tx_hash].elapsed() >= Duration::from_secs(IGNORE_PERIOD_SECS) {
                                                ignore_txs.remove(&tx_hash);
                                                break;
                                            }
                                            drop(ignore_txs);
                                            tokio::time::sleep(Duration::from_secs(1)).await;
                                        }
                                    }
                                }
                            }
                            Err(err) => {
                                // Handle the error case, for example, log the error:
                                println!("Error while making sandwich: {:?}", err);
                            }
                        }
                    }
                }
                Err(err) => {
                    println!("Error reading transaction from stream: {:?}", err);
                }
            }
        }
        Ok(())
    })
}

