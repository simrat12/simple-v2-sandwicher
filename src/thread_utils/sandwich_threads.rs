use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::collections::HashMap;
use std::error::Error;
use dashmap::DashMap;

fn initialize_class(web3: &Web3<web3::transports::Http>, global_contracts: &V2Contracts, tx: &str, upper_bound_sand: U256) -> Result<V2SwapTransaction, Box<dyn Error>> {
    V2SwapTransaction::new(web3, tx, global_contracts, upper_bound_sand).map_err(|e| {
        println!("Exception: {:?}", e);
        e.into()
    })
}

fn thread_initialize_class(web3: &Web3<web3::transports::Http>, global_contracts: &V2Contracts, pending_transactions: HashMap<String, String>, upper_bound_sand: U256) -> HashMap<String, Result<V2SwapTransaction, Box<dyn Error>>> {
    let handles: Vec<JoinHandle<()>> = Vec::new();
    let result = Arc::new(DashMap::new());

    let web3 = Arc::new(web3.clone());
    let global_contracts = Arc::new(global_contracts.clone());

    for (_, tx) in pending_transactions.into_iter() {
        let web3 = web3.clone();
        let global_contracts = global_contracts.clone();
        let tx_hash = tx.clone();
        let result_clone = Arc::clone(&result);

        let handle = thread::spawn(move || {
            let swap_tx = initialize_class(&web3, &global_contracts, &tx, upper_bound_sand);
            result_clone.insert(tx_hash, swap_tx);
        });

        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }

    result.into_iter().map(|entry| (entry.key().clone(), entry.value().clone())).collect()
}

pub fn max_sandwich_constraints(swap_dict: HashMap<String, Result<V2SwapTransaction, Box<dyn Error>>>, lower_bound_profits: i64, upper_bound_sand: u32) -> Option<V2SwapTransaction> {
    let filtered_swaps: Vec<V2SwapTransaction> = swap_dict.into_iter()
        .filter_map(|(_, res_swap)| res_swap.ok())
        .filter(|swap| swap.abstract_profits.is_some())
        .collect();

    let filtered_swaps = filtered_swaps.into_iter()
        .filter(|swap| {
            swap.abstract_profits.unwrap() > lower_bound_profits &&
            swap.delta_sand > 0 &&
            swap.delta_sand < upper_bound_sand
        })
        .collect::<Vec<_>>();

    let max_profit_swap = filtered_swaps.into_iter()
        .max_by(|a, b| a.abstract_profits.unwrap().partial_cmp(&b.abstract_profits.unwrap()).unwrap_or(std::cmp::Ordering::Equal));

    max_profit_swap
}
