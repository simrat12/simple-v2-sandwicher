use std::sync::{Arc, Mutex};
use std::thread;
use std::collections::HashMap;

struct ThreadWithReturnValue<T> {
    handle: JoinHandle<()>,
    receiver: Receiver<T>,
}

impl<T: Send + 'static> ThreadWithReturnValue<T> {
    fn new<F, A>(func: F, args: A) -> Result<Self, Box<dyn Error>>
    where
        F: FnOnce(A) -> T + Send + 'static,
        A: Send + 'static,
    {
        let (tx, rx) = mpsc::channel();
        let handle = thread::spawn(move || {
            let result = func(args);
            tx.send(result).unwrap();
        });

        Ok(Self {
            handle,
            receiver: rx,
        })
    }

    fn join(&self) -> Result<T, mpsc::RecvError> {
        self.receiver.recv()
    }
}

fn initialize_class(web3: &str, global_contracts: &str, tx: &str, upper_bound_sand: u32) -> Option<V2SwapTransaction> {
    match V2SwapTransaction::new(web3, tx, global_contracts, upper_bound_sand) {
        Ok(swap_tx) => Some(swap_tx),
        Err(e) => {
            println!("Exception: {:?}", e);
            None
        }
    }
}

fn thread_initialize_class(web3: &str, global_contracts: &str, pending_transactions: HashMap<String, String>, upper_bound_sand: u32) -> HashMap<String, Option<V2SwapTransaction>> {
    let mut handles = vec![];
    let result = Arc::new(Mutex::new(HashMap::new()));

    for (_, tx) in pending_transactions.into_iter() {
        let web3 = web3.to_string();
        let global_contracts = global_contracts.to_string();
        let tx_hash = tx.clone();
        let result_clone = Arc::clone(&result);

        let handle = thread::spawn(move || {
            let swap_tx = initialize_class(&web3, &global_contracts, &tx, upper_bound_sand);
            result_clone.lock().unwrap().insert(tx_hash, swap_tx);
        });

        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }

    Arc::try_unwrap(result).unwrap().into_inner().unwrap()
}

fn max_sandwich_constraints(swap_dict: HashMap<String, Option<V2SwapTransaction>>, lower_bound_profits: f64, upper_bound_sand: u32) -> Option<V2SwapTransaction> {
    // Remove instances with None or without abstract_profits field
    let mut filtered_swaps: Vec<V2SwapTransaction> = swap_dict.into_iter()
        .filter_map(|(_, opt_swap)| opt_swap)
        .filter(|swap| /* check if swap has an abstract_profits field, assuming it's an Option<f64> */)
        .collect();

    // Filter swaps based on criteria
    filtered_swaps.retain(|swap| {
        swap.abstract_profits.unwrap() > lower_bound_profits && 
        swap.delta_sand > 0 && 
        swap.delta_sand < upper_bound_sand
    });

    // Find the swap with the highest abstract_profits value
    let max_profit_swap = filtered_swaps.into_iter()
        .max_by(|a, b| a.abstract_profits.unwrap().partial_cmp(&b.abstract_profits.unwrap()).unwrap());

    max_profit_swap
}

