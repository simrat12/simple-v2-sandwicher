# simple-v2-sandwicher

Sandwich bot for Uniswap V2 type swaps, integrated with Uniswap V2 & V3, 1inch, Sushi, and sent with Flashbots. The key features are:

- Local simulations using ganache forks, i.e. transactions are simulated with Flashbots on submission only.
- Stale transaction filtering, e.g. checking nonces and gas prices are still valid.
-  Multithreading implementations for bundle submission / failure analysis, and for pre-processing of potential swap transactions.
- Latency monitoring and warnings.
- After the fact analysis of failed bundles, posted to a Discord channel.
- Separation of contract owner and executioner roles, with executioner address reset to 0x0000… automatically by the contract if unprofitable transactions or buy transactions that fail the validity check are signed by the executioner address.
## Dependencies
- RPC access to Ethereum node capable of creating ganache forks.
- Ganache
- Brownie (--fork.preLatestConfirmations must be set to 0)
- Flashbots Web3.py
- eth_account
- Etherscan API key
## Deployment
Use at your own risk, according to the laws in your jurisdiction, and try not to get rekt.
1.	Deploy contract with a cold wallet (owner role) and set executioner to hot wallet to be used.
2.	Send ETH to executioner wallet, and WETH to the contract.
3.	Update variables in main.py and send_bundle.py accordingly.
4.	Run generate_contracts.py, then main.py, and then send_bundle.py.
## Limitations
-	Possible for executioner to buy tokens and not sell them if the buy transaction passes validity checks.
- Possible that a successful bundle will be tampered with by a validator or reorganised such that only a buy occurs or a failed transaction lands (resetting the executioner role).
- Only WETH <-> Token swaps are possible.
- Simulations (both local or Flashbots) may not be accurate leading to a failed bundle on chain.
