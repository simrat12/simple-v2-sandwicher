use std::collections::HashMap;
use std::convert::TryInto;

use web3::contract::{Contract, Options};
use web3::types::{H160, H256, U256};
use web3::Web3;

struct V2SwapTransaction<'a> {
    upper_bound_sand: U256,
    tx: &'a web3::types::Transaction,
    w3: &'a Web3<web3::transports::Http>,
    v2_contracts: &'a V2Contracts<'a>,
    data: HashMap<&'static str, Vec<u8>>,
    pool_address: H160,
    pool_contract: Contract<web3::transports::Http>,
    amount_out: U256,
    path: Vec<H160>,
    amount_in: U256,
    router: Contract<web3::transports::Http>,
    base_token: H160,
    token_address: H160,
    r0: U256,
    r1: U256,
    base_position: u8,
    delta_sand_on_block: u64,
    pair_address: H160,
    delta_sand: U256,
    abstract_profits: i64,
}

impl<'a> V2SwapTransaction<'a> {
    fn new(
        web3: &'a Web3<web3::transports::Http>,
        tx: &'a web3::types::Transaction,
        v2_contracts: &'a V2Contracts<'a>,
        upper_bound_sand: U256,
    ) -> Self {
        let mut v2_swap_tx = V2SwapTransaction {
            upper_bound_sand,
            tx,
            w3: web3,
            v2_contracts,
            data: HashMap::new(),
            pool_address: H160::default(),
            pool_contract: Contract::default(),
            amount_out: U256::default(),
            path: Vec::new(),
            amount_in: U256::default(),
            router: Contract::default(),
            base_token: H160::default(),
            token_address: H160::default(),
            r0: U256::default(),
            r1: U256::default(),
            base_position: 0,
            delta_sand_on_block: 0,
            pair_address: H160::default(),
            delta_sand: U256::default(),
            abstract_profits: 0,
        };
        v2_swap_tx._switch();
        v2_swap_tx._max_sandwich();
        v2_swap_tx
    }

    fn _switch(&mut self) -> Result<(), String> {
        let router = self.tx.to.to_lowercase();
        if router == self.v2_contracts.uni_router.address.to_lowercase() {
            let data = self.v2_contracts.uni_router.decode_function_input(&self.tx.input)?;
            self.data = Some(data);
            self._check_uni(&self.v2_contracts.uni_router, &self.v2_contracts.uni_factory)?;
        } else if router == self.v2_contracts.sushi_router.address.to_lowercase() {
            let data = self.v2_contracts.sushi_router.decode_function_input(&self.tx.input)?;
            self.data = Some(data);
            self._check_uni(&self.v2_contracts.sushi_router, &self.v2_contracts.sushi_factory)?;
        } else if router == self.v2_contracts.shiba_router.address.to_lowercase() {
            let data = self.v2_contracts.shiba_router.decode_function_input(&self.tx.input)?;
            self.data = Some(data);
            self._check_uni(&self.v2_contracts.shiba_router, &self.v2_contracts.shiba_factory)?;
        } else if router == self.v2_contracts.v3_router.address.to_lowercase() {
            let data = self.v2_contracts.v3_router.decode_function_input(&self.tx.input)?;
            self.data = Some(data);
            self._check_v3()?;
        } else if router == self.v2_contracts.inch_router.address.to_lowercase() {
            let data = self.v2_contracts.inch_router.decode_function_input(&self.tx.input)?;
            self.data = Some(data);
            self._check_inch()?;
        } else {
            return Err(format!("Router not valid: {}", self.tx.to));
        }
        Ok(())
    }

    fn check_inch(&mut self) -> Result<(), String> {
        println!("Checking 1inch swap...");
    
        // Check that unoswap function is being called
        if self.data.0.fn_name != "unoswap" && self.data.0.fn_name != "unoswapWithPermit" {
            return Err(format!("Incorrect function {}", self.data.0.fn_name));
        }
    
        // Check that first pool is eth or dai
        if self.data.1.src_token.to_lowercase() == "0x0000000000000000000000000000000000000000".to_lowercase() {
            let base_token = self.v2_contracts.weth_contract.address;
            self.pool_address = self.w3.to_checksum_address(&format!(
                "0x{}",
                hex::encode(&self.data.1.pools[0][28..48])
            ));
        } else {
            return Err(format!("Source token is not ETH: {}", self.data.1.src_token));
        }
    
        // Get pool address and token
        let pool_contract = self.w3.eth.contract(self.pool_address, self.v2_contracts.uni_pair.abi.to_vec());
        self.amount_out = self.data.1.min_return;
    
        let token_zero = pool_contract.function("token0").unwrap().call::<String>(()).unwrap();
        let token_one = pool_contract.function("token1").unwrap().call::<String>(()).unwrap();
    
        // Assign token
        let token = if token_zero.to_lowercase() == base_token.to_lowercase() {
            token_one
        } else {
            token_zero
        };
    
        // Calculate amount out if pool > 1
        if self.data.1.pools.len() > 1 {
            // Format pool path
            let mut pool_path = HashMap::new();
            let mut p = 0;
    
            for pool in &self.data.1.pools {
                if p != 0 {
                    let pool_addr = self.w3.to_checksum_address(&format!(
                        "0x{}",
                        hex::encode(&pool[28..48])
                    ));
                    pool_path.insert(
                        p,
                        Pool {
                            contract: self.w3
                                .eth
                                .contract(pool_addr, self.v2_contracts.uni_pair.abi.to_vec()),
                            in_token: String::new(),
                            out_token: String::new(),
                        },
                    );
                }
                p += 1;
            }
    
            println!("Pool path step 1 {:?}", pool_path);
    
            // Get reserves of pool tokens
            let mut token_in = token.to_lowercase();
    
            for key in pool_path.keys() {
                let (token_0, token_1) = (
                    pool_path[key]
                        .contract
                        .function("token0")
                        .unwrap()
                        .call::<String>(())
                        .unwrap()
                        .to_lowercase(),
                    pool_path[key]
                        .contract
                        .function("token1")
                        .unwrap()
                        .call::<String>(())
                        .unwrap()
                        .to_lowercase(),
                );
    
                let pool_reserves = pool_path[key].contract.function("getReserves").unwrap().call::<(u128, u128)>(()).unwrap();
    
                let (token_0_reserve, token_1_reserve) = (pool_reserves.0, pool_reserves.1);
    
                if token_0 == token_in {
                    pool_path.get_mut(key).unwrap().in_token = token_0.clone();
                    pool_path.get_mut(key).unwrap().out_token = token_1.clone();
                    token_in = token_1;
                } else {
                    pool_path.get_mut(key).unwrap().in_token = token1.clone();
                    pool_path.get_mut(key).unwrao().out_token = token0.clone();
                }
            }

            println("Pool path step 2 {:?}", pool_path);

            let mut _id = pool_path.len();
            let mut amount_out = self.amount_out.clone();
            let virtual_router = self.v2_contracts.uni_router.clone();

            while _id > 0 {
                amount_out = virtual_router
                    .get_amount_in(amount_out, pool_path[_id - 1]["in"], pool_path[_id - 1]["out"])
                    .call()
                    .await?;
                _id -= 1;
            }

            self.amount_out = amount_out;
            println!("Calculated amount_out {}", amount_out);
            println!("Swap hash {}", self.tx["hash"]);
        }

        self.path = vec![&base_token, &token];
        self.amount_in = self.data[1]["amount"].clone();
        let factory = self.pool_contract.functions.factory().call().to_lowercase();
        if factory == self.v2_contracts.uni_factory.address.to_lowercase() {
            self.router = self.v2_contracts.uni_router.clone();
        } else if factory == self.v2_contracts.sushi_factory.address.to_lowercase() {
            self.router = self.v2_contracts.sushi_factory.clone();
        } else if factory == self.v2_contracts.shiba_factory.address.to_lowercase() {
            self.router = self.v2_contracts.shiba_router.clone();
        } else {
            panic!("Unable to assign router from factory: {}", factory);
        }
    }

    fn _check_v3(&mut self) {
        println!("Checking v3 swap...");
        // check correct function is being called
        if !["multicall"].contains(&self.data[0].fn_name.as_str()) {
            panic!("Unrecognised call: {}", self.data[0].fn_name);
        }
        // decode data
        let decoded_data = self.v2_contracts.v3_router.decode_function_input(&self.data[1]["data"][0]);
        self.data = decoded_data;
        self._check_uni(&self.v2_contracts.uni_router, &self.v2_contracts.uni_factory);
    }

    fn _check_uni(&mut self, router: &Contract, factory: &Contract) {
        self.router = router;
        println!("Checking v2 swap...");
        // check correct function is being called
        if !["swapExactETHForTokens", "swapETHForExactTokens", "swapExactTokensForETH", "swapExactTokensForExactETH", "swapExactTokensForTokens", "swapTokensForExactTokens"].contains(&self.data[0].fn_name) {
            panic!("Unrecognised function: {}", self.data[0].fn_name);
        }
        // check that first token in path is weth .. or len(path) > 1
        self.path = self.data[1]["path"];
        if self.path[0].to_lowercase() != self.v2_contracts.weth_contract.address.to_lowercase() {
            panic!("Invalid path: {:?}", self.path);
        }
        // define min amount out and path
        let func = self.data[0].fn_name.clone();
        if func == "swapETHForExactTokens" {
            self.amount_in = &mut self.tx["value"];
            self.amount_out = &mut self.data[1]["amountOut"];
        } else if func == "swapExactETHForTokens" {
            self.amount_in = &mut self.tx["value"];
            self.amount_out = &mut self.data[1]["amountOutMin"];
        } else if func == "swapExactTokensForETH" || func == "swapExactTokensForTokens" {
            self.amount_in = &mut self.data[1]["amountIn"];
            self.amount_out = &mut self.data[1]["amountOutMin"];
        } else if func == "swapExactTokensForExactETH" || func == "swapTokensForExactTokens" {
            self.amount_in = &mut self.data[1]["amountInMax"];
            self.amount_out = &mut self.data[1]["amountOut"];
        } else {
            panic!("Error setting amounts.");
        }
        // calculate amountOut for paths longer than 2
        if self.path.len() > 2 {
            let virtual_amount_out = self.amount_out.clone();
            let mut virtual_path = self.path.clone();
            virtual_path.remove(0);
            self.amount_out = self.router.functions.getAmountsIn(virtual_amount_out, &virtual_path).call()[0].clone();
        }
        // get pool address for uniswap
        self.pool_address = factory.functions.getPair(&self.path[0], &self.path[1]).call();
        // create pool contract
        self.pool_contract = self.w3.eth.contract(self.pool_address, abi=&self.v2_contracts.uni_pair.abi);
    }

    fn _max_sandwich(&mut self) {
        // check nonce is still valid
        if self.tx["nonce"].clone().parse::<u64>().unwrap() < self.w3.eth.get_transaction_count(&self.tx["from"]).unwrap() {
            panic!("Nonce too low!");
        }
        // define token
        self.base_token = self.path[0].clone();
        self.token_address = self.path[1].clone();
        let token0 = self.pool_contract.functions.token0().call().unwrap();
        let reserves = self.pool_contract.functions.getReserves().call().unwrap();
        let (r0, r1, base_position) = if token0.to_lowercase() == self.base_token.to_lowercase() {
            (to_integer_if_hex(&reserves[0]), to_integer_if_hex(&reserves[1]), 0)
        } else {
            (to_integer_if_hex(&reserves[1]), to_integer_if_hex(&reserves[0]), 1)
        };
        self.r0 = r0;
        self.r1 = r1;
        self.base_position = base_position;
        self.amount_in = to_integer_if_hex(&self.amount_in);
        self.amount_out = to_integer_if_hex(&self.amount_out);
        let amm = V2AMM::new(self.r0, self.r1, self.amount_in, self.amount_out);
        self.delta_sand_on_block = self.w3.eth.block_number().unwrap();
        self.pair_address = self.pool_address.clone();
        self.delta_sand = amm.optimal_sandwich();
        if self.delta_sand > self.upper_bound_sand {
            self.delta_sand = 0;
        }
        self.abstract_profits = amm.abstract_profits(self.delta_sand);
    }

}

struct V2AMM {
    initial_reserves_eth: i64,
    initial_reserves_token: i64,
    swap_eth_in: i64,
    swap_token_out: i64,
    fee: f64,
    min_eth_step: f64,
    eth_step: f64,
}

impl V2AMM {
    fn new(reserves_eth: i64, reserves_token: i64, swap_eth_in: i64, swap_token_out: i64) -> Self {
        let fee = 0.003;
        let min_eth_step = 0.0001;
        let mut eth_step = 0.0;
        let mut amm = V2AMM {
            initial_reserves_eth: reserves_eth,
            initial_reserves_token: reserves_token,
            swap_eth_in,
            swap_token_out,
            fee,
            min_eth_step,
            eth_step,
        };
        amm._eth_step();
        amm
    }

    fn _eth_step(&mut self) {
        if 0.5 * self.swap_eth_in as f64 * 10_f64.powi(-1) > self.min_eth_step {
            self.eth_step = 0.5 * self.swap_eth_in as f64 * 10_f64.powi(-1);
        } else {
            self.eth_step = self.min_eth_step;
        }
    }

    fn optimal_sandwich(&mut self) -> i64 {                  //Golden Search - need to check if function is unimodal
        let phi = (1.0 + (5.0 as f64).sqrt()) / 2.0;
        let tolerance = 1e-5;
    
        let mut a = 0.0;
        let mut b = self.swap_eth_in as f64;
        let mut x1 = b - (b - a) / phi;
        let mut x2 = a + (b - a) / phi;
    
        while (b - a).abs() > tolerance {
            let f1 = -1.0 * self.abstract_profits(x1 as i64);
            let f2 = -1.0 * self.abstract_profits(x2 as i64);
    
            if f1 < f2 {
                a = x1;
                x1 = x2;
                x2 = a + (b - a) / phi;
            } else {
                b = x2;
                x2 = x1;
                x1 = b - (b - a) / phi;
            }
        }
    
        ((a + b) / 2.0) as i64
    }

    fn slip_and_res(&self, delta_sand: i64) -> i64 {
        let (token_sand, mut reserves_eth, mut reserves_token) = self.swap(delta_sand, self.initial_reserves_eth, self.initial_reserves_token);
        let (token_out, new_reserves_eth, new_reserves_token) = self.swap(self.swap_eth_in, reserves_eth, reserves_token);
        reserves_eth = new_reserves_eth;
        reserves_token = new_reserves_token;
        let slippage = token_out - self.swap_token_out;
        slippage
    }
    
    fn swap(&self, input_token: i64, input_reserves_in: i64, output_reserves_in: i64) -> (i64, i64, i64) {
        let swap_fee = (input_token * self.fee as i64) / 1000;
        let invariant = input_reserves_in * output_reserves_in;
        let input_reserves_out = input_reserves_in + input_token;
        let output_reserves_out = invariant / (input_reserves_out - swap_fee);
        let output_token = output_reserves_in - output_reserves_out;
        (output_token, input_reserves_out, output_reserves_out)
    }
    
    fn abstract_profits(&self, delta_sand: i64) -> i64 {
        let (delta_token, mut reserves_eth, mut reserves_token) = self.swap(delta_sand, self.initial_reserves_eth, self.initial_reserves_token);
        let (swap_token, new_reserves_eth, new_reserves_token) = self.swap(self.swap_eth_in, reserves_eth, reserves_token);
        reserves_eth = new_reserves_eth;
        reserves_token = new_reserves_token;
        let (eth_out, new_reserves_token, new_reserves_eth) = self.swap(delta_token, reserves_token, reserves_eth);
        let abstract_profits = eth_out - delta_sand;
        abstract_profits
    }
}