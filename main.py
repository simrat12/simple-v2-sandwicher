import subprocess
import time
import pickle
import sys
from web3 import Web3, HTTPProvider
from flashbots import flashbot
from models.sandwich import Sandwich
from external_contracts.build_contracts import GlobalContracts
from thread_utils.sandwich_threads import thread_initialize_class, max_sandwich_constraints
from brownie import accounts, network, project
from brownie_utils.network_utils import change_network
from web3._utils.method_formatters import (
    to_integer_if_hex
)
from filelock import FileLock
import sched


s = sched.scheduler(time.time, time.sleep)

def clear_ignore_transactions():
    # clear ignore_transactions dictionary
    ignore_transactions.clear()
    ignore_transactions.update({"0x00": {}})
    # schedule the next run of the function after 300 seconds
    s.enter(300, 1, clear_ignore_transactions)

# schedule the first run of the function after 300 seconds
s.enter(300, 1, clear_ignore_transactions)
s.run()


# set recursion limit
sys.setrecursionlimit(10**9)

# locks
bundle_lock = FileLock('dump/bundle.txt.lock')

# load project
project.check_for_project()
project.load()
contract_part = project.ShinySporkProject

# load account and network
executor = accounts.load('mainnet_executor')
flashbots_account = accounts.load('mainnet_flashbots8')
fork_url = "http://192.168.1.32:8888"
block_provider = Web3(HTTPProvider(fork_url))
chain_id = block_provider.eth.chain_id

# load global contracts
path = 'external_contracts/v2_contracts.dictionary'
global_contracts = GlobalContracts(block_provider, path)
to_list = [global_contracts.uni_router.address,
           global_contracts.sushi_router.address,
           global_contracts.inch_router.address,
           global_contracts.v3_router.address]
to_list = [string.lower() for string in to_list]

# contract
hood_code = contract_part.hood
abi = hood_code.abi
sandwich_contract = block_provider.eth.contract(Web3.toChecksumAddress("***REMOVED***"),
                                                abi=abi)

# create pending tx filter
pending_tx_filter = block_provider.eth.filter('pending')

# delete old fork
command = f"brownie networks delete {chain_id}-fork".split()
subprocess.run(command)

# create fork
command = (f"brownie networks add development {chain_id}-fork cmd=ganache host=http://127.0.0.1 "
           + f"fork={fork_url} "
           + f"accounts=10 mnemonic=brownie port=8545 chain_id={chain_id} evm_version=arrowGlacier timeout=1").split()
subprocess.run(command)

# parameters
lower_bound_profits = 0
upper_bound_sand = 0.25 * 10 ** 18


# get filtered pending transactions
def get_pending_transactions(_new_transactions):
    _pending_transactions = {}
    for entry in _new_transactions:
        try:
            tx = block_provider.eth.get_transaction(Web3.toHex(entry))
            tx = dict(tx)
            # filtering here reduces latency and workload of main
            if str(tx['to']).lower() in to_list \
                    and int(tx['gas']) > 80000 \
                    and int(tx['value']) > 0.1 * 10 ** 18 \
                    and int(tx['nonce']) >= int(block_provider.eth.get_transaction_count(tx['from'])):
                tx['hash'] = tx['hash'].hex()
                tx['r'] = tx['r'].hex()
                tx['s'] = tx['s'].hex()
                tx_raw = block_provider.eth.get_raw_transaction(tx['hash'])
                tx['raw'] = tx_raw
                _pending_transactions[tx['hash']] = tx
                print(tx)
        except:
            pass
    return _pending_transactions


# Wait until we see the sandwiched tx on chain
def _main(_pending_transactions, _loop_start_time, _latest_block):
    _ignore_transactions = {}

    swap_dict = thread_initialize_class(block_provider, global_contracts, _pending_transactions, upper_bound_sand)

    if swap_dict:
        sandwich_tx = max_sandwich_constraints(swap_dict, lower_bound_profits, upper_bound_sand)
        if sandwich_tx is not None:
            try:
                # check nonce is still valid
                if int(sandwich_tx.tx['nonce']) < int(block_provider.eth.get_transaction_count(sandwich_tx.tx['from'])):
                    raise Exception("Nonce too low!")
                if block_provider.eth.block_number > _latest_block:
                    _latest_block = block_provider.eth.block_number
                    print(f"New block found at {_latest_block}, forking...")
                    change_network(str(chain_id) + "-fork")
                _provider = network.web3
                # change network and create flashbot object
                try:
                    if to_integer_if_hex(chain_id) == 5:
                        flashbot(_provider, flashbots_account, "https://relay-goerli.flashbots.net")
                    else:
                        flashbot(_provider, flashbots_account)
                except:
                    pass
                sandwich = Sandwich(_provider, block_provider, executor, flashbots_account, sandwich_contract,
                                    sandwich_tx, False, None)
                # check nonce is still valid
                if int(sandwich_tx.tx['nonce']) < int(block_provider.eth.get_transaction_count(sandwich_tx.tx['from'])):
                    raise Exception("Nonce too low!")
                bundle, swap_hash, real_priority_fee, bundle_hash = sandwich.make_sandwich(False, upper_bound_sand)
                print("Sandwich found!")
                print("Sandwich tx:", sandwich_tx)
                print("Bundle: ", bundle)
                print("Swap hash: ", swap_hash)
                print("Total processing time: ", time.time() - _loop_start_time)
                return _ignore_transactions, bundle, sandwich_tx.tx, _latest_block, real_priority_fee, bundle_hash
            except Exception as e:
                print("Sandwich error: ", e)
                delete_hash = sandwich_tx.tx['hash']
                _ignore_transactions[delete_hash] = _pending_transactions[delete_hash]
                print("Fork used, resetting...")
                change_network(str(chain_id) + "-fork")
                _latest_block = block_provider.eth.block_number
                return _ignore_transactions, None, None, _latest_block, None, None
    _ignore_transactions.update(_pending_transactions)
    return _ignore_transactions, None, None, _latest_block, None, None


with bundle_lock.acquire():
    with open('dump/bundle.txt', 'wb') as file:
        pickle.dump({}, file)

go = True
ignore_transactions = dict()
new_transactions = []
whole_loop_latency_vect = []
pending_transactions = {}
clear_interval = 300
clear_when = time.time()
loop_start_time = time.time()
latest_block = block_provider.eth.block_number - 1
start_block = latest_block
_bundle = None

while go:
    if block_provider.eth.block_number > latest_block:
        latest_block = block_provider.eth.block_number
        print(f"New block found at {latest_block}, forking...")
        change_network(str(chain_id) + "-fork")

    # loop start time
    whole_loop_latency = time.time() - loop_start_time
    whole_loop_latency_vect.insert(0, whole_loop_latency)
    whole_loop_latency_vect = whole_loop_latency_vect[:10]
    avg_latency = sum(whole_loop_latency_vect) / len(whole_loop_latency_vect)
    if whole_loop_latency > 0.1:
        print("WARNING! Whole loop latency: ", time.time() - loop_start_time)
    if avg_latency > 1:
        print("CRITICAL! Avg latency of last ten loops: ", avg_latency)
    loop_start_time = time.time()
    new_transactions = pending_tx_filter.get_new_entries()
    pending_transactions = get_pending_transactions(new_transactions)
    ignore_transactions, _bundle, swap_tx, latest_block, _real_priority_fee, _bundle_hash = _main(pending_transactions,
                                                                                                  loop_start_time,
                                                                                                  latest_block)
    if _bundle is not None:
        lock_start = time.time()
        with bundle_lock.acquire():
            with open('dump/bundle.txt', 'wb') as file:
                pickle.dump({'bundle': _bundle, 'swap': swap_tx, 'real_priority_fee': _real_priority_fee,
                             'bundle_hash': _bundle_hash}, file)
        if time.time() - lock_start > 0.1:
            print("WARNING! FileLock latency > 0.1s")
        print("Fork used, resetting...")
        change_network(str(chain_id) + "-fork")
        latest_block = block_provider.eth.block_number
