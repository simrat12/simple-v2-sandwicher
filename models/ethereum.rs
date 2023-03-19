use std::cmp;

pub fn next_base_fee(block_provider: &BlockProvider) -> u64 {
    let base_fee_change_denominator = 8;
    let elasticity_multiplier = 2;

    let parent_block = block_provider.eth_get_block("latest");
    let parent_base_fee = parent_block.base_fee_per_gas;
    let parent_gas_used = parent_block.gas_used;
    let parent_gas_target = parent_block.gas_limit / elasticity_multiplier;

    let base_fee = if parent_gas_used == parent_gas_target {
        parent_base_fee
    } else if parent_gas_used > parent_gas_target {
        let gas_used_delta = parent_gas_used - parent_gas_target;
        let base_fee_delta = cmp::max(
            parent_base_fee * gas_used_delta / parent_gas_target / base_fee_change_denominator,
            1,
        );
        parent_base_fee + base_fee_delta
    } else {
        let gas_used_delta = parent_gas_target - parent_gas_used;
        let base_fee_delta = parent_base_fee * gas_used_delta / parent_gas_target / base_fee_change_denominator;
        parent_base_fee - base_fee_delta
    };
    base_fee
}