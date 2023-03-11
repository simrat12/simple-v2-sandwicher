const ethers = require('ethers');
const { Hex } = require('ethers/lib/utils');
const { Contract } = require('@ethersproject/contracts');
const { FlashbotsBundleProvider } = require('@flashbots/ethers-provider-bundle');
const { BigNumber } = require('@ethersproject/bignumber');
const { ethereum } = require('@uniswap/v2-periphery');
const { getNextBlockBaseFee } = require('@ethereumjs/block');
const { utils } = require('ethers');


class Sandwich {
    constructor(web3, blockProvider, account, flashbotsAccount, sandwichContract, swapTransaction, bypass, bypassRaw) {
        this.account = account;
        this.flashbotsAccount = flashbotsAccount;
        this.contract = sandwichContract;
        this.w3 = web3;
        this.blockProvider = blockProvider;
        this.swap = swapTransaction;
        this.bypass = bypass;
        this.bypassRaw = bypassRaw;
        this._swapRaw();
        this._getBaseFee();
    }

    _swapRaw() {
        if (this.bypass) {
            this.swapRaw = this.bypassRaw;
        } else {
            this.swapRaw = this.swap.raw;
        }
    }

    async _getBaseFee() {
        const baseFee = await getNextBlockBaseFee(this.blockProvider);
        this.baseFee = baseFee.toBigInt();
        if ('gasPrice' in this.swap.tx && this.swap.tx.gasPrice !== null) {
            this.swapMaxFee = this.swap.tx.gasPrice;
            this.swapPriorityFee = this.swapMaxFee.sub(this.baseFee);
        } else {
            this.swapMaxFee = this.swap.tx.maxFeePerGas;
            this.swapPriorityFee = this.swap.tx.maxPriorityFeePerGas;
        }
        if (this.swapMaxFee.lt(this.baseFee)) {
            throw new Error('Swap transaction unable to pay base fee!');
        }
        if (this.swapMaxFee.sub(this.baseFee).lt(this.swapPriorityFee)) {
            this.swapEffectivePriorityFee = this.swapMaxFee.sub(this.baseFee);
        } else {
            this.swapEffectivePriorityFee = this.swapPriorityFee;
        }
    }

    async getSecondBreadSlice(nonce, chainId, tokenAddress, pairAddress, ethOut, paymentRatio) {
        ethOut = Hex.toHexString(ethOut).slice(2);
        while (ethOut.length < 24) {
            ethOut = '0' + ethOut;
        }
        const package1 = pairAddress + ethOut;
        paymentRatio = Hex.toHexString(paymentRatio).slice(2);
        while (paymentRatio.length < 24) {
            paymentRatio = '0' + paymentRatio;
        }
        const package2 = tokenAddress + paymentRatio;
        const sellTx = this.contract.functions.sell_for_weth(package1, package2);
        const sellTxParams = {
            gasLimit: BigNumber.from(150000),
            maxFeePerGas: this.swapMaxFee,
            maxPriorityFeePerGas: BigNumber.from(0),
            chainId: chainId,
            nonce: nonce + 1,
        };
        const sellTxSigned = await this.account.signTransaction(await sellTx.populateTransaction(sellTxParams));
        const sellHash = sellTxSigned.hash;
        return [sellTxSigned, sellHash];
    }

    async makeInversebrahOnBread(nonce, chainId, pairAddress, deltaSand, tokenOut) {
        tokenOut = Hex.toHexString(tokenOut).slice(2);
        while (tokenOut.length < 24) {
            tokenOut = '0' + tokenOut;
        }
        const packageData = pairAddress + tokenOut;
        value = deltaSand * 10 ** (-9);
        const buy_tx = this.contract.functions.buy_with_weth(packageData);
        const buy_tx_params = {
            gasLimit: BigNumber.from(150000),
            maxFeePerGas: this.swapMaxFee,
            maxPriorityFeePerGas: BigNumber.from(0),
            chainId: chainId,
            nonce: nonce + 1,
        };
        const buyTxSigned = await this.account.signTransaction(await buy_tx.populateTransaction(buy_tx_params));
        const buyHash = buyTxSigned.hash;
        return [buyTxSigned, buyHash];
    }

    async makeSandwich(testing, testingAmount) {
        // get nonce of account, chain, fees
        const chainId = await ethers.provider.getNetwork().then(n => n.chainId);
        // load weth
        const weth = new Contract(this.swap.v2Contracts.wethContract.address, this.swap.v2Contracts.wethContract.abi, provider);
        // add funds to contract if testing
        if (testing) {
          await weth.deposit({ value: testingAmount });
          await weth.transfer(contract.address, testingAmount);
          await accounts[0].sendTransaction({
            to: account.address,
            value: testingAmount,
            gasPrice: utils.parseUnits('10', 'gwei'),
          });
        }
        // get nonce and initialise eth_balance_before as very large amount
        const nonce = await account.getTransactionCount();
        // load data
        const basePosition = this.swap.basePosition;
        const pairAddress = this.swap.pairAddress;
        // set random to even if weth_position is zero
        let deltaSand = Math.floor(this.swap.deltaSand * 1e-9);
        if (basePosition === 0) {
          if (deltaSand % 2 === 1) {
            deltaSand -= 1;
          }
        } else {
          if (deltaSand % 2 === 0) {
            deltaSand -= 1;
          }
        }
        deltaSand = Math.floor(deltaSand * 1e9);
        // get other data
        const baseReserves = this.swap.r0;
        const tokenAddress = this.swap.tokenAddress;
        const tokenReserves = this.swap.r1;
        const routerContract = new Contract(this.swap.router.address, this.swap.router.abi, provider);
        const tokenOut = await routerContract.getAmountOut(deltaSand, baseReserves, tokenReserves);
        const token = new Contract(tokenAddress, weth.abi, provider);
        // make inversebrah on bread
        const [inversebrahOnBread, buyHash] = await makeInversebrahOnBread(nonce, chainId, pairAddress, deltaSand, tokenOut);
        console.log('Got inversebrah on bread');
        // format test_inversebrah_on_bread
        const simInversebrahOnBread = await ethers.provider.send('eth_signBundle', [inversebrahOnBread]);
        simInversebrahOnBread.forEach((tx, i) => {
          simInversebrahOnBread[i] = utils.hexlify(tx);
        });
      
        // require contract balance is 0, can be set to any small number (although contract will need to be edited)
        if (await ethers.provider.getBalance(contract.address) !== 0) {
          throw new Error('Contract eth balance must be 0 to sandwich.');
        }
        // require tokenOut < 2^95
        if (BigInt(tokenOut) > BigInt(2 ** 95)) {
          throw new Error('Token out too large!');
        }
        // do simulation
        let totalGasUsed = 0;
        // get signed sell_tx
        const token_balance = await token.methods.balanceOf(this.contract.address).call();
        const eth_amount_out = await router_contract.methods.getAmountsOut(
            token_balance - 42, [token_address, weth.options.address]
        ).call();
        const { rawTransaction } = await this.get_second_bread_slice(nonce, chain_id, token_address,
                                                                        pair_address, eth_amount_out, 99);
        inversebrah_on_bread.push({"signed_transaction": rawTransaction});
        // format test_inversebrah_on_bread
        const sim_sell = await this.w3.flashbots.sign_bundle([{"signed_transaction": rawTransaction}]);
        sim_sell[0] = web3.utils.hexToBytes(sim_sell[0]);
        console.log("Got sell tx");
        console.log("Contract profit: ", eth_amount_out - delta_sand);
        console.log('Simulating transaction ', 2);
        const miner_balance_before = await this.w3.eth.getBalance("0x0000000000000000000000000000000000000000");
        const { transactionHash } = await this.w3.eth.sendSignedTransaction(sim_sell[0]);
        const receipt = await this.w3.eth.getTransactionReceipt(transactionHash);
        total_gas_used += receipt.gasUsed;
        console.log("Sell gas: ", receipt.gasUsed);
        if (receipt.status == 0) {
            throw new Error("First simulation failed!");
        }
        console.log("Second simulation successful!");
        console.log("Total gas used: ", total_gas_used);
        const sandwich_payment = await this.w3.eth.getBalance("0x0000000000000000000000000000000000000000") - miner_balance_before;
        const sandwich_effective_price = sandwich_payment / total_gas_used;
        const real_priority_fee = sandwich_payment / total_gas_used;
        if (this.swap_effective_priority_fee > sandwich_effective_price) {
            console.log("Delta: ", (this.swap_effective_priority_fee - sandwich_effective_price) * 10 ** (-9), " gwei");
            throw new Error("Effective gas price too low!");
        }
        const bundle_hash = '0x' + buy_hash.slice(2) + this.swap.tx['hash'].slice(2) + sell_hash.slice(2);
        console.log("BUNDLE CONCAT", bundle_hash);
        const hashed_bundle = await this.w3.utils.soliditySha3({t: 'bytes', v: bundle_hash});
        console.log("BUNDLE HASH", hashed_bundle);
        return [inversebrah_on_bread, this.swap.tx['hash'], real_priority_fee, hashed_bundle];
    };
}

