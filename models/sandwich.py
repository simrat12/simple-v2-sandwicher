from brownie import Contract, accounts
from hexbytes import HexBytes
from models.ethereum import next_base_fee


class Sandwich:
    def __init__(self, web3, block_provider, account, flashbots_account, sandwich_contract, swap_transaction, bypass,
                 bypass_raw):
        self.account = account
        self.flashbots_account = flashbots_account
        self.contract = sandwich_contract
        self.w3 = web3
        self.block_provider = block_provider
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
        self.base_fee = next_base_fee(self.block_provider)
        # calculate gas price
        if 'gasPrice' not in self.swap.tx or self.swap.tx['gasPrice'] is None:
            self.swap_max_fee = self.swap.tx['maxFeePerGas']
            self.swap_priority_fee = self.swap.tx['maxPriorityFeePerGas']
        else:
            self.swap_max_fee = self.swap.tx['gasPrice']
            self.swap_priority_fee = self.swap_max_fee - self.base_fee
        if self.swap_max_fee < self.base_fee:
            raise Exception("Swap transaction unable to pay base fee!")
        if self.swap_max_fee - self.base_fee < self.swap_priority_fee:
            self.swap_effective_priority_fee = self.swap_max_fee - self.base_fee
        else:
            self.swap_effective_priority_fee = self.swap_priority_fee

    def get_second_bread_slice(self, nonce, chain_id, token_address, pair_address, eth_out, payment_ratio):
        eth_out = self.w3.toHex(eth_out)[2:]
        while len(eth_out) < 24:
            eth_out = '0' + eth_out
        package_1 = pair_address + eth_out
        payment_ratio = self.w3.toHex(payment_ratio)[2:]
        while len(payment_ratio) < 24:
            payment_ratio = '0' + payment_ratio
        package_2 = token_address + payment_ratio
        sell_tx = self.contract.functions.sell_for_weth(package_1, package_2)
        sell_tx = sell_tx.build_transaction({
            "gas": 150000,
            "maxFeePerGas": self.swap_max_fee,
            "maxPriorityFeePerGas": 0,
            "chainId": int(chain_id),
            "nonce": nonce + 1
        })
        sell_tx_signed = self.w3.eth.account.sign_transaction(sell_tx, self.account.private_key)
        sell_hash = sell_tx_signed.hash.hex()
        return sell_tx_signed, sell_hash

    def make_inversebrah_on_bread(self, nonce, chain_id, pair_address, delta_sand, token_out):
        token_out = self.w3.toHex(token_out)[2:]
        while len(token_out) < 24:
            token_out = '0' + token_out
        package = pair_address + token_out
        value = int(delta_sand * 10 ** (-9))
        buy_tx = self.contract.functions.buy_with_weth(package)
        buy_tx = buy_tx.build_transaction({
            "value": value,
            "gas": 150000,
            "maxFeePerGas": self.swap_max_fee,
            "maxPriorityFeePerGas": 0,
            "chainId": int(chain_id),
            "nonce": nonce,
        })
        buy_tx_signed = self.w3.eth.account.sign_transaction(buy_tx, self.account.private_key)
        buy_hash = buy_tx_signed.hash.hex()
        inversebrah_on_bread = [{"signed_transaction": buy_tx_signed.rawTransaction},
                                {"signed_transaction": self.swap_raw}]
        return inversebrah_on_bread, buy_hash

    def make_sandwich(self, testing, testing_amount):
        # get nonce of account, chain, fees
        chain_id = self.w3.eth.chain_id
        # load weth
        weth = self.swap.v2_contracts.weth_contract
        weth = Contract.from_abi(name="weth", address=weth.address, abi=weth.abi)
        # add funds to contract if testing
        if testing:
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
        router_contract = Contract.from_abi(name="router", address=self.swap.router.address, abi=self.swap.router.abi)
        token_out = router_contract.getAmountOut(delta_sand, base_reserves, token_reserves)
        token = Contract.from_abi(name="token", address=token_address, abi=weth.abi)
        # make inversebrah on bread
        inversebrah_on_bread, buy_hash = self.make_inversebrah_on_bread(nonce=nonce, chain_id=chain_id,
                                                                        pair_address=pair_address,
                                                                        delta_sand=delta_sand,
                                                                        token_out=token_out)
        print("Got inversebrah on bread")
        # format test_inversebrah_on_bread
        sim_inversebrah_on_bread = self.w3.flashbots.sign_bundle(inversebrah_on_bread)
        i = 0
        while i < len(sim_inversebrah_on_bread):
            sim_inversebrah_on_bread[i] = HexBytes(sim_inversebrah_on_bread[i])
            i += 1

        # require contract balance is 0, can be set to any small number (although contract will need to be edited)
        if int(self.w3.eth.get_balance(self.contract.address)) != 0:
            raise Exception("Contract eth balance must be 0 to sandwich.")
        # require token_out < 2^95
        if int(token_out) > int(2 ** 95):
            raise Exception("Token out too large!")
        # do simulation
        total_gas_used = 0
        for i in sim_inversebrah_on_bread:
            print('Simulating transaction ', sim_inversebrah_on_bread.index(i))
            receipt = self.w3.eth.send_raw_transaction(i)
            receipt = dict(self.w3.eth.wait_for_transaction_receipt(receipt.hex()))
            if sim_inversebrah_on_bread.index(i) == 0:
                total_gas_used += receipt['gasUsed']
                print("Buy gas: ", total_gas_used)
            if receipt['status'] == 0:
                raise Exception("First simulation failed!")
        print("First simulation successful")
        # get signed sell_tx
        token_balance = token.balanceOf(self.contract.address)
        eth_amount_out = router_contract.getAmountsOut(token_balance - 42, [token_address, weth.address])[1]
        sell_tx_signed, sell_hash = self.get_second_bread_slice(nonce=nonce, chain_id=chain_id,
                                                                token_address=token_address,
                                                                pair_address=pair_address,
                                                                eth_out=eth_amount_out,
                                                                payment_ratio=99)
        inversebrah_on_bread.append({"signed_transaction": sell_tx_signed.rawTransaction})
        # format test_inversebrah_on_bread
        sim_sell = self.w3.flashbots.sign_bundle([{"signed_transaction": sell_tx_signed.rawTransaction}])
        sim_sell[0] = HexBytes(sim_sell[0])
        print("Got sell tx")
        print("Contract profit: ", eth_amount_out - delta_sand)
        print('Simulating transaction ', 2)
        miner_balance_before = self.w3.eth.get_balance("0x0000000000000000000000000000000000000000")
        receipt = self.w3.eth.send_raw_transaction(sim_sell[0])
        receipt = dict(self.w3.eth.wait_for_transaction_receipt(receipt.hex()))
        total_gas_used += receipt['gasUsed']
        print("Sell gas: ", receipt['gasUsed'])
        if receipt['status'] == 0:
            raise Exception("First simulation failed!")
        print("Second simulation successful!")
        print("Total gas used: ", total_gas_used)
        sandwich_payment = self.w3.eth.get_balance("0x0000000000000000000000000000000000000000") - miner_balance_before
        sandwich_effective_price = sandwich_payment / total_gas_used
        real_priority_fee = sandwich_payment / total_gas_used
        if self.swap_effective_priority_fee > sandwich_effective_price:
            print("Delta: ", (self.swap_effective_priority_fee - sandwich_effective_price) * 10 ** (-9), " gwei")
            raise Exception("Effective gas price too low!")
        bundle_hash = '0x' + buy_hash[2:] + self.swap.tx['hash'][2:] + sell_hash[2:]
        print("BUNDLE CONCAT", bundle_hash)
        bundle_hash = self.w3.keccak(hexstr=bundle_hash).hex()
        print("BUNDLE HASH", bundle_hash)
        return inversebrah_on_bread, self.swap.tx['hash'], real_priority_fee, bundle_hash
