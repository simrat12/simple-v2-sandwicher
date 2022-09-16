from threading import Thread
from models.v2_models import V2SwapTransaction
from operator import itemgetter


class ThreadWithReturnValue(Thread):
    def __init__(self, group=None, target=None, name=None,
                 args=(), kwargs=None):
        Thread.__init__(self, group, target, name, args, kwargs)
        if kwargs is None:
            kwargs = {}
        self._kwargs = kwargs
        self._args = args
        self._target = target
        self._return = None

    def run(self):
        if self._target is not None:
            self._return = self._target(*self._args,
                                        **self._kwargs)

    def join(self, *args):
        Thread.join(self, *args)
        return self._return


def inititialize_class(web3, global_contracts, tx, upper_bound_sand):
    try:
        swap_tx = V2SwapTransaction(web3, tx, global_contracts, upper_bound_sand)
    except Exception as e:
        swap_tx = None
        print("Exception: ", e)
    return swap_tx


def thread_initialize_class(web3, global_contracts, pending_transactions, upper_bound_sand):
    result = dict()
    thread_refs = dict()
    for _, tx in pending_transactions.items():
        # double check to_list and check nonce still valid
        tx_hash = tx['hash']
        tx_thread = ThreadWithReturnValue(target=inititialize_class, args=(web3,
                                                                           global_contracts,
                                                                           tx,
                                                                           upper_bound_sand))
        tx_thread.start()
        thread_refs[tx_hash] = tx_thread
    for tx_hash, tx_thread in thread_refs.items():
        result[tx_hash] = tx_thread.join()
    return result


def max_sandwich_constraints(swap_dict, lower_bound_profits, upper_bound_sand):
    # delete swaps with empty max_result
    delete_list = []
    for element in swap_dict:
        if not swap_dict[element] or not hasattr(swap_dict[element], 'abstract_profits'):
            delete_list.append(element)
    for element in delete_list:
        del swap_dict[element]
    # create swap list
    swap_list = [swap_dict[element] for element in swap_dict
                 if swap_dict[element].abstract_profits > lower_bound_profits
                 and upper_bound_sand > swap_dict[element].delta_sand > 0
                 ]
    # create profit size index
    if swap_list:
        profits_swap_list = [element.abstract_profits for element in swap_list]
        if profits_swap_list:
            index, max_profits = max(enumerate(profits_swap_list), key=itemgetter(1))
            return swap_list[index]
        else:
            return None
    else:
        return None
