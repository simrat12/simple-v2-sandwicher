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

contract hood{
    // parameters
    address public immutable owner;
    address public immutable weth_store;
    address public executioner;

    // constructors
    constructor(address _owner, address _executioner, address _weth){
        owner = _owner;
        weth_store = _weth;
        executioner = _executioner;
    }

    // modifiers
    modifier OnlyExecutioner(){ require(msg.sender==executioner); _; }

    modifier OnlyOwner(){ require(msg.sender==owner); _; }

    // setters

    // setExecutioner, method ***REMOVED***
    function ***REMOVED***(address name) public OnlyOwner{
        executioner = name;
    }

    // sendETH, method ***REMOVED***
    function ***REMOVED***() public payable OnlyOwner{}

    receive() external payable{
        require(msg.sender == weth_store, 'not weth');
    }

    fallback() external payable{
        require(msg.sender == weth_store, 'not weth');
    }

    // Main functions
    // functions for buy and sell

    // payment funciton
    // make_payment, method
    function make_payment(uint payment_ratio, uint profit, uint gas_fees, uint sand) internal {
        uint payment = payment_ratio * (profit / 100);
        // withdraw all weth (gas refund)
        require(payment <= profit, 'profit');
        IERC20(weth_store).withdraw(payment + gas_fees);
        payable(block.coinbase).transfer(payment);
        payable(executioner).transfer(gas_fees + sand);
    }

    // Buy trade
    // Even delta if base position zero
    // packed format: bytes32(abi.encodePacked(address pair, uint token_out))
    // buy_with_weth, method ***REMOVED***
    function ***REMOVED***(bytes32 package) public payable OnlyExecutioner{
        // decode call data
        address pair = address(bytes20(package));
        uint token_out = uint96(bytes12(package << 160));
        // get address, balance and sand
        address hood_add = address(this);
        uint hood_bal = hood_add.balance;
        uint sand = msg.value;
        // require zero balance, otherwise delete executioner
        if (hood_bal - sand != 0){
            executioner = 0x0000000000000000000000000000000000000000;
        }
        IERC20(weth_store).transfer(pair, sand * 10 ** 9);
        if (sand % 2 == 0){
            IUniswapV2Pair(pair).swap(0, token_out, hood_add,"");
        }
        else{
            IUniswapV2Pair(pair).swap(token_out, 0, hood_add,"");
        }
    }

    // Sell trade
    // more call with both addresses here
    // sell_for_weth, method ***REMOVED***
    function ***REMOVED***(bytes32 package_1, bytes32 package_2) public OnlyExecutioner{
        // decode call data
        address pair = address(bytes20(package_1));
        uint eth_out = uint96(bytes12(package_1 << 160));
        address token = address(bytes20(package_2));
        uint payment_ratio = uint96(bytes12(package_2 << 160));
        // get address, balance and sand
        address hood_address = address(this);
        uint hood_bal = hood_address.balance;
        uint delta_sand = hood_bal * 10 ** 9;
        // swap
        IERC20(token).transfer(pair, IERC20(token).balanceOf(hood_address) - 42);
        if (hood_bal % 2 == 0){
            IUniswapV2Pair(pair).swap(eth_out, 0, hood_address,"");
        }
        else{
            IUniswapV2Pair(pair).swap(0, eth_out, hood_address,"");
        }
        uint gas_fees = 200000 * block.basefee;
        uint profits = (eth_out - delta_sand) - gas_fees;
        if (profits != 0){
            // make payment
            make_payment(payment_ratio, profits, gas_fees, hood_bal);
        }
        else{
            executioner = 0x0000000000000000000000000000000000000000;
        }
    }

    // Allows withdrawal of ETH and tokens
    function  withdrawToken(address _token,uint _amount) public
    OnlyOwner{ IERC20(_token).transfer(owner, _amount); }

    // withdrawWETHtoETH, method
    function  withdrawWETHtoETH(uint _amount) public OnlyOwner{
        IERC20(weth_store).withdraw(_amount);
        payable(owner).transfer(_amount);
    }

    function withdrawWETHtoETHtoExecutioner(uint _amount) public OnlyOwner{
        IERC20(weth_store).withdraw(_amount);
        payable(executioner).transfer(_amount);
    }

    function withdrawETH( uint _amount) public OnlyOwner{ payable(owner).transfer(_amount); }

}
