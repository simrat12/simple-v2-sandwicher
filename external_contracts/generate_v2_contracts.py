import json

from brownie import Contract, network
from brownie_utils.network_utils import change_network
import os

# set network
change_network('homeETH')
web3 = network.web3

# export etherscan api token
os.environ["ETHERSCAN_TOKEN"] = ""


def build_entry(contract):
    return {'abi': contract.abi, 'address': contract.address, 'bytecode': contract.bytecode,
            'signatures': contract.signatures, 'topics': contract.topics}


class V2Contract:
    def __init__(self, router_address):
        self._contracts(router_address)
        self._build_dictionary()

    def _contracts(self, router_address):
        self.router_contract = Contract.from_explorer(router_address)
        self.weth_contract = Contract.from_explorer(self.router_contract.WETH())
        self.factory_contract = Contract.from_explorer(self.router_contract.factory())
        self.pair_contract = Contract.from_explorer(self.factory_contract.allPairs(0))

    def _build_dictionary(self):
        self.dictionary = {
            'router': build_entry(self.router_contract),
            'weth': build_entry(self.weth_contract),
            'factory': build_entry(self.factory_contract),
            'pair': build_entry(self.pair_contract)
        }


class V2CompatibleRouter:
    def __init__(self, router_address):
        self._build_dictionary(router_address)

    def _build_dictionary(self, router_address):
        self.dictionary = {
            'router': build_entry(Contract.from_explorer(router_address))
        }


# create v2 contract objects from router address
uni = V2Contract("0x7a250d5630B4cF539739dF2C5dAcb4c659F2488D")
sushi = V2Contract("0xd9e1cE17f2641f24aE83637ab66a2cca9C378B9F")
shiba = V2Contract("0x03f7724180AA6b939894B5Ca4314783B0b36b329")

# create v2 compatible contract objects from router address
v3 = V2CompatibleRouter("0x68b3465833fb72A70ecDF485E0e4C7bD8665Fc45")
inch = V2CompatibleRouter("0x1111111254fb6c44bAC0beD2854e76F90643097d")

# create models contract dictionary from router address
v2_dictionary = {
    'uni': uni.dictionary,
    'sushi': sushi.dictionary,
    'shiba': shiba.dictionary,
    'v3': v3.dictionary,
    'inch': inch.dictionary,
    'weth': {'contract': uni.dictionary['weth']}
}

# save to file
with open('v2_contracts.dictionary', 'w') as file:
    json.dump(v2_dictionary, file)
