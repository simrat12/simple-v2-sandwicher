// SPDX-License-Identifier: MIT
pragma solidity 0.8.16;

interface IUniswapV2Pair {
    function swap(uint amount0Out, uint amount1Out, address to, bytes calldata data) external;
}

interface IERC20 {
    function transfer(address to, uint value) external returns (bool);
    function withdraw(uint) external;
    function balanceOf(address account) external view returns (uint256);
}

contract hood {
    // hard code zero value used in buy / sell function, and the amount of gas to be refunded to executioner per valid
    // sell transaction
    uint immutable zero = 0;
    uint immutable gas_amount = 200000;
    address immutable zero_address = 0x0000000000000000000000000000000000000000;
    address public immutable owner;
    address public immutable weth_store;
    address public executioner;

    // owner and executioner specified during deployment, contract can be deployed from any address
    constructor(address _owner, address _executioner, address _weth){
        owner = _owner;
        weth_store = _weth;
        executioner = _executioner;
    }

    // enables function restrictions for owner or executioner
    modifier OnlyOwner(){ require(msg.sender==owner); _; }
    modifier OnlyExecutioner(){ require(msg.sender==executioner); _; }

    // allows executioner role to be reset by owner
    function setExecutioner(address name) public OnlyOwner{
        executioner = name;
    }

    // only owner and WETH are allowed to send ETH to the contract
    // these restrictions are important for the buy function
    function sendETH() public payable OnlyOwner{}
    receive() external payable{
        require(msg.sender == weth_store, 'not weth');
    }
    fallback() external payable{
        require(msg.sender == weth_store, 'not weth');
    }

    // main functions

    // payment function
    function make_payment(uint payment_ratio, uint profit, uint gas_fees, uint sand) internal {
        uint payment = payment_ratio * (profit / 100);
        require(payment <= profit, 'profit');
        // sense checks profit < payment (in the event of a squiffy payment_ratio) and then withdraws payment + gas
        // fees from WETH
        IERC20(weth_store).withdraw(payment + gas_fees);
        // payment sent to miner, gas_fees sent to executioner
        payable(block.coinbase).transfer(payment);
        payable(executioner).transfer(gas_fees + sand);
    }

    // buy trade
    // package format: bytes32(abi.encodePacked(address pair, uint token_out))
    // pair and token_out amounts computed offline
    // do NOT call this function outside of a bundle that has been simulated correctly, if the pair address is wrong or
    // a malicious contract, all weth from the contract might be lost
    function buy_with_weth(bytes32 package) public payable OnlyExecutioner{
        // decode call data
        address pair = address(bytes20(package));
        uint token_out = uint96(bytes12(package << 160));
        // get address and balance of the contract
        address hood_add = address(this);
        uint hood_bal = hood_add.balance;
        // gets sand from msg.value, which is the size of the eth trade scaled by 10 ** (-9)
        // sand being even or odd specifies the weth position in the swap pair
        // scaling sand incorrectly will make the transaction fail (either wrong weth position or incorrect token_out
        uint sand = msg.value;
        // require zero balance, zero can be set to any small value, assists with logic of the sell function
        // deletes executioner if it fails
        if (hood_bal - sand != zero){
            executioner = zero_address;
        }
        else {
            // optimistically transfer weth to pair for swap
            IERC20(weth_store).transfer(pair, sand * 10 ** 9);
            // using modulo to get weth position from sand, and then doing the swap
            if (sand % 2 == 0){
                IUniswapV2Pair(pair).swap(0, token_out, hood_add,"");
            }
            else{
                IUniswapV2Pair(pair).swap(token_out, 0, hood_add,"");
            }
        }
    }

    // sell trade
    // packages format: bytes32(abi.encodePacked(address, uint))
    // pair, eth_out, token, and payment ratio computed offline
    // do NOT call this function outside of a bundle that has been simulated correctly, losses could occur
    function sell_for_weth(bytes32 package_1, bytes32 package_2) public OnlyExecutioner{
        // decode call data
        address pair = address(bytes20(package_1));
        uint eth_out = uint96(bytes12(package_1 << 160));
        address token = address(bytes20(package_2));
        uint payment_ratio = uint96(bytes12(package_2 << 160));
        // get address, balance and sand
        // delta_sand is the buy amount coded with msg.value in the buy transaction
        address hood_address = address(this);
        uint hood_bal = hood_address.balance;
        uint delta_sand = hood_bal * 10 ** 9;
        // reset executioner if token is WETH
        if (token == weth_store) {
            executioner = zero_address;
        }
        else{
            // swap all tokens except small amount
            IERC20(token).transfer(pair, IERC20(token).balanceOf(hood_address) - 42);
            // decodes weth position from balance, as in the buy transaction
            if (hood_bal % 2 == 0){
                IUniswapV2Pair(pair).swap(eth_out, 0, hood_address,"");
            }
            else{
                IUniswapV2Pair(pair).swap(0, eth_out, hood_address,"");
            }
            // calculates gas fees to be refunded to executioner from base fee
            uint gas_fees = gas_amount * block.basefee;
            // calculates total profits for the contract, will revert if underflow occurs (i.e. negative profit)
            uint profits = (eth_out - delta_sand) - gas_fees;
            // resets executioner if zero profits, otherwise makes payment
            if (profits != 0){
                // make payment
                make_payment(payment_ratio, profits, gas_fees, hood_bal);
            }
            else{
                executioner = zero_address;
            }
        }
    }

    // allows withdrawal of ETH and tokens to owner
    function  withdrawToken(address _token, uint _amount) public
    OnlyOwner{ IERC20(_token).transfer(owner, _amount); }
    function  withdrawWETHtoETH(uint _amount) public OnlyOwner{
        IERC20(weth_store).withdraw(_amount);
        payable(owner).transfer(_amount);
    }
    function withdrawETH( uint _amount) public OnlyOwner{ payable(owner).transfer(_amount); }
}
