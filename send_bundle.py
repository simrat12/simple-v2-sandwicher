import time
import pickle
import requests
import json
import random
import sys
import datetime
from brownie import network, accounts
from flashbots import flashbot
from eth_account.signers.local import LocalAccount
from eth_account.account import Account
from brownie_utils.network_utils import change_network
import threading
from filelock import FileLock
from web3._utils.method_formatters import (
    to_integer_if_hex
)

# set recursion limit
sys.setrecursionlimit(10**9)

# discord
token = ''
channel = ''
baseURL = "https://discordapp.com/api/channels/{}/messages".format(channel)
headers = {"Authorization": "Bot {}".format(token), "Content-Type": "application/json", }

# locks
bundle_lock = FileLock('dump/bundle.txt.lock')
results_lock = FileLock('dump/results.txt.lock')

time_stamp_local_adjustment = -3600
change_network('homeGETH')
goerli = False
flashbots_account = accounts.load('mainnet_flashbots8')
executor = accounts.load('mainnet_executor')
signer: LocalAccount = Account.from_key(flashbots_account.private_key)
provider = network.web3

# create flashbot object
if goerli:
    flashbot(provider, signer, "https://relay-goerli.flashbots.net")
else:
    flashbot(provider, signer)


def _print(_data):
    print(_data)
    json_data = json.dumps({"content": _data})
    requests.post(baseURL, headers=headers, data=json_data)


def bundle_failure(swap_tx, real_priority_fee, bundle_hash, time_stamps):
    time.sleep(12)
    time.sleep(random.randint(1, 10))
    separator = "~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~\n"
    print_data = separator
    swap_hash = swap_tx["hash"]
    print_data += f"Bundle failure analysis for swap: {swap_hash}\nBundle hash: {bundle_hash}\n"
    # get mined transaction
    try:
        swap_dict = dict(provider.eth.get_transaction(swap_hash))
        swap_block = swap_dict["blockNumber"]
    except Exception as e:
        print(f"Unable to find swap transaction, error: {e}\n")
        return
    if swap_dict["blockNumber"] is None:
        print("Swap transaction was not mined.\n")
        return
    # get bundle stats
    block_time_stamp = time_stamps[swap_block] + time_stamp_local_adjustment
    print_data += f"Block {swap_block} seen by node at: {block_time_stamp}\n"
    bundle_stats = dict(provider.flashbots.get_bundle_stats(bundle_hash, swap_block))
    submitted_at = datetime.datetime.timestamp(datetime.datetime.strptime(bundle_stats["submittedAt"],
                                                                          "%Y-%m-%dT%H:%M:%S.%fZ"))
    print_data += f"Submitted bundle at time: {submitted_at}, {block_time_stamp - submitted_at}s before block\n"
    if not bundle_stats["isSimulated"]:
        print_data += "Bundle was not simulated.\n"
        _print(print_data + separator)
        return
    simulated_at = datetime.datetime.timestamp(datetime.datetime.strptime(bundle_stats["simulatedAt"],
                                                                          "%Y-%m-%dT%H:%M:%S.%fZ"))
    print_data += f"Simulated bundle at time: {simulated_at}, {block_time_stamp - simulated_at}s before block\n"
    if not bundle_stats["isSentToMiners"]:
        print_data += "Bundle was not sent to miners.\n"
    else:
        to_miners_at = datetime.datetime.timestamp(datetime.datetime.strptime(bundle_stats["sentToMinersAt"],
                                                                              "%Y-%m-%dT%H:%M:%S.%fZ"))
        print_data += f"Sent to miners at time: {to_miners_at}, {block_time_stamp - to_miners_at}s before block\n"
    # check if fb block
    get_fb_block = 0
    fb_blocks = {}
    while get_fb_block < 10:
        get_fb_block += 1
        try:
            fb_blocks = requests.get(url='https://blocks.flashbots.net/v1/blocks',
                                     params={'block_number': swap_dict["blockNumber"]})
            fb_blocks = fb_blocks.json()
            if len(fb_blocks["blocks"]) > 1:
                break
        except Exception as e:
            if get_fb_block == 10:
                print_data += f"Unable to request block from FB: {e}\n"
                _print(print_data + separator)
                return
        time.sleep(60)
    if len(fb_blocks["blocks"]) < 1:
        print_data += f"{swap_block} not a flashbots block.\n"
        _print(print_data + separator)
        return
    # check if tx was in bundle
    fb_transactions = fb_blocks["blocks"][swap_dict["blockNumber"]]["transactions"]
    in_bundle = False
    bundle_index = 0
    for transaction in fb_transactions:
        if swap_tx['hash'].lower() == transaction["transaction_hash"].lower():
            in_bundle = True
            bundle_index = transaction["bundle_index"]
            break
        else:
            in_bundle = False
    if not in_bundle:
        print_data += "Swap transaction not in a bundle.\n"
        _print(print_data + separator)
        return
    # construct bundle
    competitor_bundle = []
    for transaction in fb_transactions:
        if bundle_index == transaction["bundle_index"]:
            competitor_bundle.append(transaction)
    if len(competitor_bundle) != 3:
        print_data += f"Swap transaction was part of large bundle: {competitor_bundle}\n"
        _print(print_data + separator)
        return
    # calculate effective gas price of competitor
    competitor_gas = int(competitor_bundle[0]["gas_used"]) + int(competitor_bundle[2]["gas_used"])
    competitor_payment = int(competitor_bundle[0]["total_miner_reward"]) + int(
        competitor_bundle[2]["total_miner_reward"])
    competitor_effective_priority_fee = competitor_payment / competitor_gas
    print_data += f"Priority fee delta: {(competitor_effective_priority_fee - real_priority_fee) * 10 ** (-9)} gwei\n"
    print_data += f"Priority fee delta ratio: {(competitor_effective_priority_fee / real_priority_fee) * 100}%\n"
    print_data += f"Competitor bundle: {competitor_bundle}\n"
    _print(print_data + separator)


def send_bundle(bundle, swap_tx, real_priority_fee, bundle_hash):
    # keep trying to send bundle until it gets mined
    start_block = provider.eth.block_number
    swap_nonce = swap_tx['nonce']
    swap_sender = swap_tx['from']
    time_stamps = {start_block: time.time()}
    executor_nonce = provider.eth.get_transaction_count(executor.address)
    while True:
        block = provider.eth.block_number
        target_block = block + 1
        print(f"Testing nonce on {block}")
        # check swap tx nonce
        if swap_nonce < provider.eth.get_transaction_count(swap_sender):
            time.sleep(12)
            try:
                transaction = provider.eth.get_transaction(swap_tx['hash'])
            except:
                transaction = "Replaced"
            print("Nonce too low: ", transaction)
            break
        # send bundle targeting next block
        fb_latency = time.time()
        print(f"Sending bundle targeting block {target_block}")
        send_result = provider.flashbots.send_bundle(bundle, target_block_number=target_block)
        print("FB latency: ", time.time() - fb_latency, "s")
        # wait for next block to mine
        while provider.eth.block_number == block:
            time.sleep(0.1)
        time_stamps[to_integer_if_hex(target_block)] = time.time()
        if executor_nonce != provider.eth.get_transaction_count(executor.address):
            try:
                send_result.wait()
                receipts = send_result.receipts()
                print(f"\nBundle was mined in block {receipts[0].blockNumber}\a")
                print("Receipt: ", receipts)
                return
            except:
                print(f"Bundle not found in block {target_block}")
        else:
            print(f"Bundle not found in block {target_block}")
        if provider.eth.chain_id == 1 and block > start_block + 10:
            print(f"\nBundle was not mined within 10 blocks.")
            break
        if provider.eth.chain_id == 5 and block > start_block + 1000:
            print(f"\nBundle was not mined within 100 blocks.")
            break
    bundle_failure(swap_tx, real_priority_fee, bundle_hash, time_stamps)


with bundle_lock.acquire():
    with open('dump/bundle.txt', 'wb') as file:
        pickle.dump({}, file)
print("Executor balance ", executor.balance() * 10 ** (-18), "ETH")
while True:
    time.sleep(0.1)
    with bundle_lock.acquire():
        with open('dump/bundle.txt', 'rb') as file:
            data = pickle.load(file)
    if data:
        _bundle = data['bundle']
        _swap_tx = data['swap']
        _bundle_hash = data['bundle_hash']
        _real_priority_fee = data['real_priority_fee']
        threading.Thread(target=send_bundle, args=(_bundle, _swap_tx, _real_priority_fee, _bundle_hash)).start()
        lock_start = time.time()
        with bundle_lock.acquire():
            with open('dump/bundle.txt', 'wb') as file:
                pickle.dump({}, file)
        if time.time() - lock_start > 0.1:
            print("WARNING! FileLock latency > 0.1s")
