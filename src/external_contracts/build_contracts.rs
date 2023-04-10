use std::fs::File;
use std::io::BufReader;
use std::collections::HashMap;
use std::error::Error;
use serde_json::Value;
use web3::contract::{Contract, ContractError};
use web3::eth::Eth;
use web3::types::Address;

struct GlobalContracts {
    web3: Eth,
    path: String,
    contracts: HashMap<String, Contract<Eth>>,
}

impl GlobalContracts {
    fn new(web3: Eth, path: String) -> Result<Self, Box<dyn Error>> {
        let mut contracts = HashMap::new();
        let file = File::open(&path)?;
        let reader = BufReader::new(file);
        let contracts_json: Value = serde_json::from_reader(reader)?;

        for (protocol, contract_map) in contracts_json.as_object().unwrap().iter() {
            for (contract, data) in contract_map.as_object().unwrap().iter() {
                let address: Address = data["address"].as_str().unwrap().parse()?;
                let abi = data["abi"].clone();
                let key = format!("{}_{}", protocol, contract);
                let contract_instance = Contract::from_json(web3.clone(), address, abi.to_string().as_bytes())?;
                contracts.insert(key, contract_instance);
            }
        }

        Ok(Self {
            web3,
            path,
            contracts,
        })
    }
}
