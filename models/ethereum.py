def next_base_fee(block_provider):
    base_fee_change_denominator = 8
    elasticity_multiplier = 2

    parent_block = dict(block_provider.eth.get_block('latest'))
    parent_bas_fee = parent_block["baseFeePerGas"]
    parent_gas_used = parent_block["gasUsed"]
    parent_gas_target = parent_block["gasLimit"] / elasticity_multiplier

    if parent_gas_used == parent_gas_target:
        base_fee = parent_bas_fee
    elif parent_gas_used > parent_gas_target:
        gas_used_delta = parent_gas_used - parent_gas_target
        base_fee_delta = max(parent_bas_fee * gas_used_delta // parent_gas_target // base_fee_change_denominator, 1)
        base_fee = parent_bas_fee + base_fee_delta
    else:
        gas_used_delta = parent_gas_target - parent_gas_used
        base_fee_delta = parent_bas_fee * gas_used_delta // parent_gas_target // base_fee_change_denominator
        base_fee = parent_bas_fee - base_fee_delta
    return base_fee
