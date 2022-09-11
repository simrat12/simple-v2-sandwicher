import json


class GlobalContracts:
    def __init__(self, web3, path):
        self.w3 = web3
        self.path = path
        self._build_contracts()

    def _build_contracts(self):
        self.contracts = dict()
        with open(self.path) as file:
            contracts = json.load(file)
        for protocol in contracts.keys():
            for contract in contracts[protocol].keys():
                address = contracts[protocol][contract]['address']
                abi = contracts[protocol][contract]['abi']
                code = f"self.{protocol}_{contract} = self.w3.eth.contract(address='{address}', abi={abi})"
                exec(code)
