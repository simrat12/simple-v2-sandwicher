import time
import pickle
from brownie import network, accounts
from flashbots import flashbot
from eth_account.signers.local import LocalAccount
from eth_account.account import Account
from brownie_utils.network_utils import change_network
import requests

change_network('homeGETH')
goerli = False
flashbots_account = accounts.load('mainnet_flashbots6')
executor = accounts.load('executor_vanity1')
signer: LocalAccount = Account.from_key(flashbots_account.private_key)
provider = network.web3

# create flashbot object
if goerli:
    flashbot(provider, signer, "https://relay-goerli.flashbots.net")
else:
    flashbot(provider, signer)


def send_to_ethermine(bundle, block):
    formatted_bundle = []
    for i in bundle:
        tx = i["signed_transaction"].hex()[4:]
        formatted_bundle.append(tx)
    print("Formatted bundle: ", formatted_bundle)
    try:
        json_data = {"jsonrpc": "2.0", "method": "eth_sendBundle",
                     "params": {
                         "txs": formatted_bundle,
                         "blockNumber": provider.toHex(block),
                         "minTimestamp": "0x0",
                         "maxTimestamp": "0x0"
                     }, "id": 1337}
        print("json data", json_data)
        json_call = requests.post("https://mev-relay.ethermine.org", json=json_data).json()
        print("Sent bundle to Ethermine: ", json_call["result"])
    except Exception as e:
        print("Error sending bundle to Ethermine: ", e)


def send_bundle(bundle, swap_tx):
    # keep trying to send bundle until it gets mined
    start_block = provider.eth.block_number
    swap_nonce = swap_tx['nonce']
    swap_sender = swap_tx['from']
    executor_nonce = provider.eth.get_transaction_count(executor.address)
    while True:
        block = provider.eth.block_number
        simulate_start = time.time()
        print(f"Simulating on block {block}")
        if block > start_block + 5:
            # simulate bundle on current block
            try:
                provider.flashbots.simulate(bundle, block)
                print("Simulation time: ", time.time() - simulate_start)
                print("Simulation successful.")
            except Exception as e:
                print("Simulation time: ", time.time() - simulate_start)
                print("Simulation error", e)
                return {"block": block, "success": False}
        # check swap tx nonce
        if swap_nonce < provider.eth.get_transaction_count(swap_sender):
            print("Nonce too low: ", provider.eth.get_transaction(swap_tx['hash']))
            return {"block": block, "success": False}
        # send bundle targeting next block
        print(f"Sending bundle targeting block {block + 1}")
        send_result = provider.flashbots.send_bundle(bundle, target_block_number=block + 1)
        # send_to_ethermine(bundle, block)
        while provider.eth.block_number == block:
            time.sleep(0.1)
        if executor_nonce != provider.eth.get_transaction_count(executor.address):
            try:
                send_result.wait()
                receipts = send_result.receipts()
                print(f"\nBundle was mined in block {receipts[0].blockNumber}\a")
                return {"block": receipts[0].blockNumber, "success": True}
            except:
                print(f"Bundle not found in block {block + 1}")
        else:
            print(f"Bundle not found in block {block + 1}")
        if provider.eth.chain_id == 1 and block > start_block + 10:
            print(f"\nBundle was not mined within 10 blocks.")
            return {"block": block, "success": False}
        if provider.eth.chain_id == 5 and block > start_block + 1000:
            print(f"\nBundle was not mined within 100 blocks.")
            return {"block": block, "success": False}


with open('dump/results.txt', 'wb') as file:
    pickle.dump({}, file)

with open('dump/bundle.txt', 'wb') as file:
    pickle.dump({}, file)

while True:
    time.sleep(0.1)
    with open('dump/bundle.txt', 'rb') as file:
        data = pickle.load(file)
    if data:
        _bundle = data['bundle']
        _swap_tx = data['swap']
        results = send_bundle(_bundle, _swap_tx)
        with open('dump/results.txt', 'wb') as file:
            pickle.dump(results, file)
        with open('dump/bundle.txt', 'wb') as file:
            pickle.dump({}, file)
