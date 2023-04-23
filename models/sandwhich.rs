use ethers::prelude::*;
use std::convert::TryInto;
use web3::types::{TransactionReceipt, H256, U256};

struct Sandwich {
    block_provider: BlockProvider,
    account: Account,
    flashbots_account: FlashbotsAccount,
    sandwich_contract: Contract,
    swap_transaction: V2SwapTransaction,
    bypass: bool,
    bypass_raw: Vec<u8>,
}

pub struct SandwichResult {
    pub inversebrah_on_bread: Vec<RawTransaction>,
    pub swap_hash: H256,
    pub real_priority_fee: U256,
    pub bundle_hash: H256,
}

impl Sandwich {
    async fn new(
        block_provider: Web3BlockProvider,
        account: Account,
        flashbots_account: Account,
        sandwich_contract: Contract<Http>,
        swap_transaction: SwapTransaction,
        bypass: bool,
        bypass_raw: Vec<u8>,
    ) -> Self {
        let mut sandwich = Self {
            account,
            flashbots_account,
            contract: sandwich_contract,
            block_provider,
            swap: swap_transaction,
            bypass,
            bypass_raw,
            swap_raw: vec![],
            base_fee: U256::zero(),
            swap_max_fee: U256::zero(),
            swap_priority_fee: U256::zero(),
            swap_effective_priority_fee: U256::zero(),
        };

        sandwich._swap_raw();
        sandwich._get_base_fee().await;

        sandwich
    }

    fn _swap_raw(&mut self) {
        self.swap_raw = if self.bypass {
            self.bypass_raw.clone()
        } else {
            self.swap.tx.raw.clone()
        };
    }

    async fn _get_base_fee(&mut self) {
        self.base_fee = next_base_fee(&self.block_provider).await;

        if let Some(gas_price) = self.swap.tx.gas_price {
            self.swap_max_fee = gas_price;
            self.swap_priority_fee = self.swap_max_fee - self.base_fee;
        } else {
            self.swap_max_fee = self.swap.tx.max_fee_per_gas.unwrap();
            self.swap_priority_fee = self.swap.tx.max_priority_fee_per_gas.unwrap();
        }

        if self.swap_max_fee < self.base_fee {
            panic!("Swap transaction unable to pay base fee!");
        }

        if self.swap_max_fee - self.base_fee < self.swap_priority_fee {
            self.swap_effective_priority_fee = self.swap_max_fee - self.base_fee;
        } else {
            self.swap_effective_priority_fee = self.swap_priority_fee;
        }
    }

    async fn get_second_bread_slice(
        &self,
        nonce: u64,
        chain_id: u64,
        token_address: Address,
        pair_address: Address,
        eth_out: U256,
        payment_ratio: U256,
    ) -> (Transaction, H256) {
        let eth_out_hex = eth_out.to_hex_string();
        let eth_out_hex = format!("{:0>24}", eth_out_hex);

        let package_1 = format!("{}{}", pair_address.to_string(), eth_out_hex);

        let payment_ratio_hex = payment_ratio.to_hex_string();
        let payment_ratio_hex = format!("{:0>24}", payment_ratio_hex);

        let package_2 = format!("{}{}", token_address.to_string(), payment_ratio_hex);

        let sell_tx = self.contract.method::<_, Bytes>("sell_for_weth", (package_1, package_2)).await.unwrap();

        let sell_tx = sell_tx.build_transaction()
            .gas(150000)
            .max_fee_per_gas(self.swap_max_fee)
            .max_priority_fee_per_gas(U256::zero())
            .chain_id(chain_id.try_into().unwrap())
            .nonce(nonce + 1)
            .transaction();

        let sell_tx_signed = self.account.sign_transaction(sell_tx).await.unwrap();
        let sell_hash = sell_tx_signed.hash();

        (sell_tx_signed, sell_hash)
    }

    
    async fn deposit_and_transfer(
        weth: &Contract<Http>,
        contract: &Contract<Http>,
        account: &LocalWallet,
        testing_amount: U256,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let gas_price = U256::exp10(12);

        // WETH deposit
        let deposit_tx = weth
            .method::<_, _, ()>("deposit", ())
            .unwrap()
            .value(testing_amount)
            .gas_price(gas_price)
            .from(account.address())
            .send()
            .await?;

        deposit_tx.await?;

        // WETH transfer
        let transfer_tx = weth
            .method::<_, _, ()>("transfer", (contract.address(), testing_amount))
            .unwrap()
            .gas_price(gas_price)
            .from(account.address())
            .send()
            .await?;

        transfer_tx.await?;

        // Account transfer
        let provider = weth.provider().clone();
        let transfer = provider.send_transaction(
            TransactionRequest {
                to: Some(account.address()),
                value: Some(testing_amount),
                gas_price: Some(gas_price),
                ..Default::default()
            },
            Some(account.clone()),
        )
        .await?;

        transfer.await?;

        Ok(())
    }

    async fn make_inversebrah_on_bread(
        &self,
        nonce: u64,
        chain_id: u64,
        pair_address: Address,
        delta_sand: U256,
        token_out: U256,
    ) -> (Vec<RawTransaction>, H256) {
        let token_out_hex = token_out.to_hex_string();
        let token_out_hex = format!("{:0>24}", token_out_hex);

        let package = format!("{}{}", pair_address.to_string(), token_out_hex);

        let value = delta_sand / U256::exp10(9);
        let buy_tx = self.contract.method::<_, Bytes>("buy_with_weth", package).await.unwrap();

        let buy_tx = buy_tx.build_transaction()
            .value(value)
            .gas(150000)
            .max_fee_per_gas(self.swap_max_fee)
            .max_priority_fee_per_gas(U256::zero())
            .chain_id(chain_id.try_into().unwrap())
            .nonce(nonce)
            .transaction();

        let buy_tx_signed = self.account.sign_transaction(buy_tx).await.unwrap();
        let buy_hash = buy_tx_signed.hash();

        let inversebrah_on_bread = vec![
            RawTransaction::from_signed(&buy_tx_signed).unwrap(),
            RawTransaction::from_bytes(self.swap_raw.clone()).unwrap(),
        ];

        (inversebrah_on_bread, buy_hash)
    }

    // Other methods like `make_sandwich` go here
    pub async fn make_sandwich(&self, testing: bool, testing_amount: U256) -> Result<SandwichResult, Box<dyn std::error::Error>> {
        // Get nonce of account, chain, and fees
        let chain_id = self.block_provider.provider().chain_id().await?;
        let weth = &self.swap.v2_contracts.weth_contract;

        // Add funds to contract if testing
        if testing {
            deposit_and_transfer(&weth, &self.contract, &self.account, testing_amount).await?;
        }

        // Get nonce and initialize eth_balance_before
        let nonce = self.block_provider.provider().get_transaction_count(self.account.address(), None).await?;
        let base_position = self.swap.base_position;
        let pair_address = self.swap.pair_address.clone();
        let mut delta_sand = self.swap.delta_sand;
        let provider = Provider::<Http>::try_from("http://localhost:8545")?;

        // Instantiate Router contract
        let router_contract = Contract::new(router_address, weth_abi.clone(), provider.clone());

    
        let mut delta_sand_u64 = delta_sand.as_u64();
    
        if self.swap.base_position == 0 {
            if delta_sand_u64 % 2 == 1 {
                delta_sand_u64 -= 1;
            }
        } else {
            if delta_sand_u64 % 2 == 0 {
                delta_sand_u64 -= 1;
            }
        }
    
        let delta_sand = delta_sand_u64 * 10_u64.pow(9);
    
        let base_reserves = self.swap.r0;
        let token_reserves = self.swap.r1;
        let token_address = self.swap.token_address;
    
        let token_out: U256 = router_contract
            .query::<U256, _, _, _, _>("getAmountOut", (delta_sand, base_reserves, token_reserves), None, Default::default(), None)
            .await?;
    
        let token = Contract::new(&token_address, weth_abi.clone(), provider.clone());

        let (inversebrah_on_bread, buy_hash) = self.make_inversebrah_on_bread(nonce, chain_id.as_u64(), pair_address.clone(), delta_sand, token_out).await?;

        // Other parts of the make_sandwich method go here
            // Initialize the Flashbots middleware
        let flashbots_middleware =
        FlashbotsMiddleware::new(provider.clone(), self.flashbots_account.clone(), "https://relay.flashbots.net".parse()?);
        let flashbots_provider = provider.with_middleware(flashbots_middleware);

        // Create the Flashbots bundle
        let bundle = FlashbotsBundle::new(inversebrah_on_bread.clone())
            .set_simulation_block(provider.get_block_number().await?);

        // Simulate the bundle
        let simulation = flashbots_provider.simulate_bundle(&bundle).await?;

        // Check contract balance
        let contract_balance = provider.get_balance(self.contract.address, None).await?;
        if contract_balance != U256::zero() {
            return Err(Box::new(std::io::Error::new(
                std::io::ErrorKind::Other,
                "Contract eth balance must be 0 to sandwich.",
            )));
        }

        // Check token_out
        if token_out > U256::exp10(95) {
            return Err(Box::new(std::io::Error::new(
                std::io::ErrorKind::Other,
                "Token out too large!",
            )));
        }

        // Check if the first transaction failed
        if !simulation.is_successful() {
            return Err(Box::new(std::io::Error::new(
                std::io::ErrorKind::Other,
                "First simulation failed!",
            )));
        }

        // Print gas usage
        println!("Buy gas: {}", simulation.total_gas_used());

        println!("First simulation successful");
        let token_address = self.swap.token_address;

        // Get signed sell_tx
        let token_balance = token.balance_of(self.contract.address()).await?;
        let eth_amount_out = router_contract.get_amounts_out(token_balance - U256::from(42), &[token_address, weth.address()]).await?[1];
        let (sell_tx_signed, sell_hash) = self.get_second_bread_slice(nonce, chain_id.as_u64(), &token_address, &pair_address, eth_amount_out, U256::from(99)).await?;

        inversebrah_on_bread.push(RawTransaction::from_signed(&sell_tx_signed).map_err(|e| format!("Failed to convert signed transaction to raw transaction: {}", e))?);

        // Simulate the sell transaction
        let sim_sell = flashbots_provider.sign_bundle(vec![sell_tx_signed.clone()]).await?;
        println!("Got sell tx");
        println!("Contract profit: {:?}", eth_amount_out - delta_sand);
        println!("Simulating transaction 2");

        let zero_address: Address = "0x0000000000000000000000000000000000000000".parse()?;
        let miner_balance_before = self.block_provider.provider().eth().balance(zero_address.unwrap(), None).await?;
        let receipt = self.block_provider.provider().eth().send_raw_transaction(sim_sell[0].clone()).await?;
        let receipt = self.block_provider.provider().eth().transaction_receipt(receipt).await?;
        let total_gas_used = total_gas_used + receipt.gas_used.ok_or("Failed to get gas used from transaction receipt")?;
        println!("Sell gas: {:?}", receipt.gas_used.ok_or("Failed to get gas used from transaction receipt"));

        if receipt.status == Some(TransactionStatus::Failed) {
            panic!("First simulation failed!");
        }

        println!("Second simulation successful!");
        println!("Total gas used: {:?}", total_gas_used);
        let sandwich_payment = self.block_provider.provider().eth().balance(zero_address.unwrap(), None).await? - miner_balance_before;
        let sandwich_effective_price = sandwich_payment / total_gas_used;
        let real_priority_fee = sandwich_payment / total_gas_used;

        if self.swap_effective_priority_fee > sandwich_effective_price {
            println!("Delta: {} gwei", (self.swap_effective_priority_fee - sandwich_effective_price) * U256::from(10).pow(9u64.into()));
            panic!("Effective gas price too low!");
        }

        let bundle_hash = format!("0x{}{}{}", buy_hash, self.swap.tx.hash, sell_hash);
        let bundle_hash = self.block_provider.provider().keccak_hex(bundle_hash)?;

        Ok(SandwichResult {
            inversebrah_on_bread,
            swap_hash: self.swap.tx.hash,
            real_priority_fee,
            bundle_hash,
        })
    }

}


