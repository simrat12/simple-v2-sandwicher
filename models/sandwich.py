from brownie import Contract, accounts
from hexbytes import HexBytes


class Sandwich:
    def __init__(self, web3, account, flashbots_account, sandwich_contract, swap_transaction, bypass, bypass_raw):
        self.account = account
        self.flashbots_account = flashbots_account
        self.contract = sandwich_contract
        self.w3 = web3
        self.swap = swap_transaction
        self.bypass = bypass
        self.bypass_raw = bypass_raw
        self._swap_raw()
        self._get_base_fee()

    def _swap_raw(self):
        if self.bypass:
            self.swap_raw = self.bypass_raw
        else:
            self.swap_raw = self.swap.tx['raw']

    def _get_base_fee(self):
        # calculate gas price
        if 'gasPrice' not in self.swap.tx or self.swap.tx['gasPrice'] is None:
            self.get_base_fee = self.swap.tx['maxFeePerGas']
        else:
            self.get_base_fee = self.swap.tx['gasPrice']

    def get_second_bread_slice(self, nonce, chain_id, token_address, pair_address):
        sell_tx = self.contract.functions.loimsadm(pair_address, token_address)
        sell_tx = sell_tx.build_transaction({
            "gas": 150000,
            "maxFeePerGas": self.get_base_fee,
            "maxPriorityFeePerGas": 0,
            "chainId": int(chain_id),
            "nonce": nonce + 1
        })
        sell_tx_signed = self.w3.eth.account.sign_transaction(sell_tx, self.account.private_key)
        return sell_tx_signed

    def make_inversebrah_on_bread(self, nonce, chain_id, pair_address, delta_sand, token_out):
        token_out = self.w3.toHex(token_out)[2:]
        while len(token_out) < 24:
            token_out = '0' + token_out
        package = pair_address + token_out
        print("package", package)
        print("delta_sand", delta_sand)
        value = int(delta_sand * 10 ** (-9))
        print("value", value)
        buy_tx = self.contract.functions.***REMOVED***(package)
        buy_tx = buy_tx.build_transaction({
            "value": value,
            "gas": 150000,
            "maxFeePerGas": self.get_base_fee,
            "maxPriorityFeePerGas": 0,
            "chainId": int(chain_id),
            "nonce": nonce,
        })
        buy_tx_signed = self.w3.eth.account.sign_transaction(buy_tx, self.account.private_key)
        inversebrah_on_bread = [{"signed_transaction": buy_tx_signed.rawTransaction},
                                {"signed_transaction": self.swap_raw}]
        return inversebrah_on_bread

    def make_sandwich(self, testing, testing_amount):
        # get nonce of account, chain, fees
        chain_id = self.w3.eth.chain_id
        # add funds to contract if testing
        if testing:
            # load weth
            weth = self.swap.v2_contracts.weth_contract
            weth = Contract.from_abi(name="weth", address=weth.address, abi=weth.abi)
            weth.deposit({'from': accounts[0], 'value': testing_amount, 'gas_price': 10 ** 12})
            weth.transfer(self.contract.address, testing_amount, {'from': accounts[0], 'gas_price': 10 ** 12})
            accounts[0].transfer(self.account.address, testing_amount, gas_price=10 ** 12)
        # get nonce and initialise eth_balance_before as very large amount
        nonce = self.w3.eth.getTransactionCount(self.account.address)
        # load data
        base_position = self.swap.base_position
        pair_address = self.swap.pair_address
        # set random to even if weth_position is zero
        delta_sand = int(self.swap.delta_sand * 10 ** (-9))
        if base_position == 0:
            if delta_sand % 2 == 1:
                delta_sand -= 1
        else:
            if delta_sand % 2 == 0:
                delta_sand -= 1
        delta_sand = int(delta_sand * 10 ** 9)
        # get other data
        base_reserves = self.swap.r0
        token_address = self.swap.token_address
        token_reserves = self.swap.r1
        print("TOKEN RESERVES: ", token_reserves)
        print("BASE RESERVES: ", base_reserves)
        router_contract = Contract.from_abi(name="router", address=self.swap.router.address, abi=self.swap.router.abi)
        token_out = router_contract.getAmountOut(delta_sand, base_reserves, token_reserves)
        # make inversebrah on bread
        inversebrah_on_bread = self.make_inversebrah_on_bread(nonce=nonce, chain_id=chain_id,
                                                              pair_address=pair_address,
                                                              delta_sand=delta_sand,
                                                              token_out=token_out)
        print("Got inversebrah on bread")
        # get signed sell_tx
        sell_tx_signed = self.get_second_bread_slice(nonce=nonce, chain_id=chain_id,
                                                     token_address=token_address,
                                                     pair_address=pair_address)
        inversebrah_on_bread.append({"signed_transaction": sell_tx_signed.rawTransaction})
        print("Got sell tx")
        # format test_inversebrah_on_bread
        print(inversebrah_on_bread)
        sim_inversebrah_on_bread = self.w3.flashbots.sign_bundle(inversebrah_on_bread)
        i = 0
        while i < len(sim_inversebrah_on_bread):
            sim_inversebrah_on_bread[i] = HexBytes(sim_inversebrah_on_bread[i])
            i += 1

        # require contract balance is 42
        if int(self.w3.eth.get_balance(self.contract.address)) != 0:
            raise Exception("Contract eth balance must be 0 to sandwich.")
        # require token_out < 2^95
        if int(token_out) > int(2 ** 95):
            raise Exception("Token out too large!")
        # do simulation
        for i in sim_inversebrah_on_bread:
            print('Sending transaction: ', sim_inversebrah_on_bread.index(i))
            receipt = self.w3.eth.send_raw_transaction(i)
            receipt = dict(self.w3.eth.wait_for_transaction_receipt(receipt.hex()))
            if receipt['status'] == 0:
                raise Exception("Simulation failed!")
        print("Simulation successful!")
        return inversebrah_on_bread, self.swap.tx['hash']
