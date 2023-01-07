import copy

from web3._utils.method_formatters import (
    to_integer_if_hex
)


class V2SwapTransaction:
    def __init__(self, web3, tx, v2_contracts, upper_bound_sand):
        self.upper_bound_sand = upper_bound_sand
        self.tx = tx
        self.w3 = web3
        self.v2_contracts = v2_contracts
        self._switch()
        self._max_sandwich()

    def _switch(self):
        router = self.tx['to'].lower()
        if router == self.v2_contracts.uni_router.address.lower():
            self.data = self.v2_contracts.uni_router.decode_function_input(self.tx['input'])
            self._check_uni(self.v2_contracts.uni_router, self.v2_contracts.uni_factory)
        elif router == self.v2_contracts.sushi_router.address.lower():
            self.data = self.v2_contracts.sushi_router.decode_function_input(self.tx['input'])
            self._check_uni(self.v2_contracts.sushi_router, self.v2_contracts.sushi_factory)
        elif router == self.v2_contracts.shiba_router.address.lower():
            self.data = self.v2_contracts.shiba_router.decode_function_input(self.tx['input'])
            self._check_uni(self.v2_contracts.shiba_router, self.v2_contracts.shiba_factory)
        elif router == self.v2_contracts.v3_router.address.lower():
            self.data = self.v2_contracts.v3_router.decode_function_input(self.tx['input'])
            self._check_v3()
        elif router == self.v2_contracts.inch_router.address.lower():
            self.data = self.v2_contracts.inch_router.decode_function_input(self.tx['input'])
            self._check_inch()
        else:
            raise Exception("Router not valid: ", self.tx['to'])

    def _check_inch(self):
        print("Checking 1inch swap...")
        # check that unoswap function is being called
        if self.data[0].fn_name not in ["unoswap", "unoswapWithPermit"]:
            raise Exception("Incorrect function ", self.data[0].fn_name)
        # check that first pool is eth or dai
        if self.data[1]['srcToken'].lower() == "0x0000000000000000000000000000000000000000".lower():
            base_token = self.v2_contracts.weth_contract.address
        else:
            raise Exception("Source token is not ETH:", self.data[1]['srcToken'])
        # get pool address and token
        self.pool_address = self.w3.toChecksumAddress("0x" + self.data[1]['pools'][0].hex()[-40:])
        # create pool contract
        self.pool_contract = self.w3.eth.contract(self.pool_address, abi=self.v2_contracts.uni_pair.abi)
        self.amount_out = self.data[1]['minReturn']
        token_zero = self.pool_contract.functions.token0().call()
        token_one = self.pool_contract.functions.token1().call()
        # assign token
        if token_zero.lower() == base_token:
            token = token_one
        else:
            token = token_zero
        # calc amount out if pool > 1
        if len(self.data[1]['pools']) > 1:
            # format pool path
            pool_path = {}
            p = 0
            for pool in self.data[1]['pools']:
                if p != 0:
                    pool_addr = self.w3.toChecksumAddress("0x" + pool.hex()[-40:])
                    pool_path[p] = {'contract': self.w3.eth.contract(pool_addr, abi=self.v2_contracts.uni_pair.abi)}
                p += 1
            print("Pool path step 1 ", pool_path)
            # get reserves of pool tokens
            token_in = token.lower()
            for key in pool_path.keys():
                token_0 = pool_path[key]['contract'].functions.token0().call().lower()
                token_1 = pool_path[key]['contract'].functions.token1().call().lower()
                pool_reserves = pool_path[key]['contract'].functions.getReserves().call()
                token_0_reserve = pool_reserves[0]
                token_1_reserve = pool_reserves[1]
                if token_0 == token_in:
                    pool_path[key]['in'] = token_0_reserve
                    pool_path[key]['out'] = token_1_reserve
                    token_in = token_1
                else:
                    pool_path[key]['in'] = token_1_reserve
                    pool_path[key]['out'] = token_0_reserve
                    token_in = token_0
            print("Pool path step 2 ", pool_path)
            # calculate token amount out
            _id = len(pool_path)
            amount_out = copy.deepcopy(self.amount_out)
            virtual_router = self.v2_contracts.uni_router
            while _id > 0:
                amount_out = virtual_router.functions.getAmountIn(amount_out, pool_path[_id]['in'],
                                                                  pool_path[_id]['out']).call()
                _id -= 1
            self.amount_out = amount_out
            print("Calculated amount_out ", amount_out)
            print("Swap hash ", self.tx['hash'])
        # continue check
        self.path = [base_token, token]
        self.amount_in = self.data[1]['amount']
        factory = self.pool_contract.functions.factory().call().lower()
        if factory == self.v2_contracts.uni_factory.address.lower():
            self.router = self.v2_contracts.uni_router
        elif factory == self.v2_contracts.sushi_factory.address.lower():
            self.router = self.v2_contracts.sushi_factory
        elif self.v2_contracts.shiba_factory.address.lower():
            self.router = self.v2_contracts.shiba_router
        else:
            raise Exception("Unable to assign router from factory: ", factory)

    def _check_v3(self):
        print("Checking v3 swap...")
        # check correct function is being called
        if self.data[0].fn_name not in ['multicall']:
            raise Exception("Unrecognised call: ", self.data[0].fn_name)
        # decode data
        self.data = self.v2_contracts.v3_router.decode_function_input(self.data[1]['data'][0])
        self._check_uni(self.v2_contracts.uni_router, self.v2_contracts.uni_factory)

    def _check_uni(self, router, factory):
        self.router = router
        print("Checking v2 swap...")
        # check correct function is being called
        if self.data[0].fn_name not in ['swapExactETHForTokens', 'swapETHForExactTokens',
                                        'swapExactTokensForETH', 'swapExactTokensForExactETH',
                                        'swapExactTokensForTokens', 'swapTokensForExactTokens']:
            raise Exception("Unrecognised function: ", self.data[0].fn_name)
        # check that first token in path is weth .. or len(path) > 1
        self.path = self.data[1]['path']
        if self.path[0].lower() not self.v2_contracts.weth_contract.address.lower():
            raise Exception("Invalid path: ", self.path)
        # define min amount out and path
        func = self.data[0].fn_name
        if func == 'swapETHForExactTokens':
            self.amount_in = self.tx['value']
            self.amount_out = self.data[1]['amountOut']
        elif func == 'swapExactETHForTokens':
            self.amount_in = self.tx['value']
            self.amount_out = self.data[1]['amountOutMin']
        elif func == 'swapExactTokensForETH' or func == 'swapExactTokensForTokens':
            self.amount_in = self.data[1]['amountIn']
            self.amount_out = self.data[1]['amountOutMin']
        elif func == 'swapExactTokensForExactETH' or func == 'swapTokensForExactTokens':
            self.amount_in = self.data[1]['amountInMax']
            self.amount_out = self.data[1]['amountOut']
        else:
            raise Exception("Error setting amounts.")
        # calculate amountOut for paths longer than 2
        if len(self.path) > 2:
            virtual_amount_out = copy.deepcopy(self.amount_out)
            virtual_path = copy.deepcopy(self.path)
            virtual_path.pop(0)
            self.amount_out = router.functions.getAmountsIn(virtual_amount_out, virtual_path).call()[0]
        # get pool address for uniswap
        self.pool_address = factory.functions.getPair(self.path[0], self.path[1]).call()
        # create pool contract
        self.pool_contract = self.w3.eth.contract(self.pool_address, abi=self.v2_contracts.uni_pair.abi)

    def _max_sandwich(self):
        # check nonce is still valid
        if int(self.tx['nonce']) < int(self.w3.eth.get_transaction_count(self.tx['from'])):
            raise Exception("Nonce too low!")
        # define token
        self.base_token = self.path[0]
        self.token_address = self.path[1]
        token0 = self.pool_contract.functions.token0().call()
        reserves = self.pool_contract.functions.getReserves().call()
        if token0.lower() == self.base_token.lower():
            r0 = reserves[0]
            r1 = reserves[1]
            base_position = 0
        else:
            r0 = reserves[1]
            r1 = reserves[0]
            base_position = 1
        self.r0 = to_integer_if_hex(r0)
        self.r1 = to_integer_if_hex(r1)
        self.base_position = base_position
        self.amount_in = to_integer_if_hex(self.amount_in)
        self.amount_out = to_integer_if_hex(self.amount_out)
        amm = V2AMM(self.r0, self.r1, self.amount_in, self.amount_out)
        self.delta_sand_on_block = self.w3.eth.block_number
        self.pair_address = self.pool_address
        self.delta_sand = amm.optimal_sandwich()
        if self.delta_sand > self.upper_bound_sand:
            self.delta_sand = 0
        self.abstract_profits = amm.abstract_profits(self.delta_sand)


class V2AMM:
    def __init__(self, reserves_eth, reserves_token, swap_eth_in, swap_token_out):
        self.initial_reserves_eth = int(reserves_eth)
        self.initial_reserves_token = int(reserves_token)
        self.swap_eth_in = int(swap_eth_in)
        self.swap_token_out = int(swap_token_out)
        self.fee = 0.003
        self.min_eth_step = 0.0001
        self._eth_step()

    def _eth_step(self):
        if 0.5 * self.swap_eth_in * 10 ** (-1) > self.min_eth_step:
            self.eth_step = 0.5 * self.swap_eth_in * 10 ** (-1)
        else:
            self.eth_step = self.min_eth_step

    def optimal_sandwich(self):
        # calculate initial slippage
        actual_token_out = self.swap(self.swap_eth_in, self.initial_reserves_eth, self.initial_reserves_token)[0]
        slippage = actual_token_out - self.swap_token_out
        # return zero if no initial slippage
        if slippage - 1 <= 0:
            return 0
        # find delta_sand starting with delta_sand = swap_eth_in
        new_delta_sand = 0
        delta_sand = new_delta_sand
        abstract_profits = 0
        iterations = 0
        abstract_profits_list = []
        step_size = self.eth_step
        while slippage - 1 > 0:
            # doubles step size every 100 iterations
            if iterations % 100 == 99:
                step_size = step_size * 2
            delta_sand = new_delta_sand
            # calculate delta_sand +/- step
            delta_sand_u = new_delta_sand + step_size
            # calculate new slippages and reserves
            slippage_u = self.slip_and_res(delta_sand_u)
            # calculate new abstract profits
            abstract_profits_u = self.abstract_profits(delta_sand_u)
            # stops runaway iteration
            if slippage == slippage_u:
                print("Stopped runaway iteration!")
                slippage_u = 0
            # slippage logic
            if slippage_u - 1 > 0 and iterations < 3:
                abstract_profits_list.append(abstract_profits_u)
                slippage, new_delta_sand, abstract_profits = slippage_u, delta_sand_u, abstract_profits_u
            elif slippage_u - 1 > 0 and abstract_profits_u > abstract_profits_list[iterations - 2]:
                abstract_profits_list.append(abstract_profits_u)
                slippage, new_delta_sand, abstract_profits = slippage_u, delta_sand_u, abstract_profits_u
            else:
                slippage = 0
            # log iteration
            iterations += 1
        print(f"Delta_sand found within {iterations} iterations.")
        return int(delta_sand)

    def slip_and_res(self, delta_sand):
        token_sand, reserves_eth, reserves_token = self.swap(delta_sand, self.initial_reserves_eth,
                                                             self.initial_reserves_token)
        token_out, reserves_eth, reserves_token = self.swap(self.swap_eth_in, reserves_eth, reserves_token)
        slippage = token_out - self.swap_token_out
        return int(slippage)

    def swap(self, input_token, input_reserves_in, output_reserves_in):
        swap_fee = int(input_token * self.fee)
        invariant = input_reserves_in * output_reserves_in
        input_reserves_out = input_reserves_in + input_token
        output_reserves_out = int(invariant / (input_reserves_out - swap_fee))
        output_token = output_reserves_in - output_reserves_out
        return int(output_token), int(input_reserves_out), int(output_reserves_out)

    def abstract_profits(self, delta_sand):
        delta_token, reserves_eth, reserves_token = self.swap(delta_sand, self.initial_reserves_eth,
                                                              self.initial_reserves_token)
        swap_token, reserves_eth, reserves_token = self.swap(self.swap_eth_in, reserves_eth, reserves_token)
        eth_out, reserves_token, reserves_eth = self.swap(delta_token, reserves_token, reserves_eth)
        return int(eth_out - delta_sand)
