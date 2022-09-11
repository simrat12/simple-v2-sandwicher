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

    // store other parameters in one slot struct
    struct Trade {
        uint64 random;
        uint32 probability_a;
        uint32 probability_b;
        uint32 min_payment_num;
        uint32 min_payment_den;
        uint32 max_payment_num;
        uint32 max_payment_den;
    }
    Trade public trade_store;

    // constructors
    constructor(address _owner, address _executioner, address _weth, uint32[2] memory ratioMaxPayment,
    uint32[2] memory ratioMinPayment){
        owner = _owner;
        weth_store = _weth;
        executioner = _executioner;
        trade_store.random = 42;
        trade_store.probability_a = 2;
        trade_store.probability_b = 1;
        trade_store.max_payment_num = ratioMaxPayment[0];
        trade_store.max_payment_den = ratioMaxPayment[1];
        trade_store.min_payment_num = ratioMinPayment[0];
        trade_store.min_payment_den = ratioMinPayment[1];
    }

    // modifiers
    modifier OnlyExecutioner(){ require(msg.sender==executioner); _; }

    modifier OnlyOwner(){ require(msg.sender==owner); _; }

    // setters
    // setRandom, method 00000006
    function exetapql(uint64 random) public OnlyOwner{ trade_store.random = random; }

    // setPayment, method 00000009
    function lgubjnlo(uint32[2] calldata ratioMaxPayment, uint32[2] calldata ratioMinPayment) public OnlyOwner{
        (trade_store.max_payment_num, trade_store.max_payment_den) = (ratioMaxPayment[0], ratioMaxPayment[1]);
        (trade_store.min_payment_num, trade_store.min_payment_den) = (ratioMinPayment[0], ratioMinPayment[1]);
    }

    // setProbability, method 0000000c
    function hhcixswz(uint32[2] calldata probability) public OnlyExecutioner{
        (trade_store.probability_a, trade_store.probability_b) = (probability[0], probability[1]);
    }

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
    // resets sandwich
    // beep_boop_beep, method 00000002
    function tfizihhn() public OnlyExecutioner{
        if (address(this).balance != 0){
            (bool success, ) = payable(weth_store).call{value: (address(this).balance), gas: 50000}("");
            require(success, 'deposit');
        }
    }

    // functions for buy and sell
    // get_reserves, method 0000000e
    function vvcdhivk(address pair, address weth_address, address token) internal view returns (uint reserve_weth,
        uint reserve_token) {
        reserve_weth = IERC20(weth_address).balanceOf(pair);
        reserve_token = IERC20(token).balanceOf(pair);
    }

    // getAmountOut, method ***REMOVED***
    function ubmbrnxp(uint amountIn, uint reserveIn, uint reserveOut) internal pure returns (uint amountOut) {
        uint amountInWithFee = mul(amountIn, 997);
        uint numerator = mul(amountInWithFee, reserveOut);
        uint denominator = add(mul(reserveIn, 1000), amountInWithFee);
        amountOut = numerator / denominator;
    }

    // standard SafeMath add, sub, and mul
    function add(uint x, uint y) internal pure returns (uint z) {
        require((z = x + y) > x, 'add');
    }

    function sub(uint x, uint y) internal pure returns (uint z) {
        require((z = x - y) < x, 'sub');
    }

    function mul(uint x, uint y) internal pure returns (uint z) {
        require(y == 0 || (z = x * y) / y == x, 'mul');
    }

    // payment funciton
    // make_payment, method
    function make_payment(uint profit, uint gas_fees, uint sand) internal {
        // load trade from storage
        Trade memory trade = trade_store;
        // declare payment and check ran variable
        uint payment;
        if(trade.random == 9 ){
            if (random_variable([trade.probability_a, trade.probability_b])){
                payment = mul(trade.max_payment_num, profit / trade.max_payment_den);
            }
            else{ payment = mul(trade.min_payment_num, profit / trade.min_payment_den); }
        }
        else{ payment = mul(trade.min_payment_num, profit / trade.min_payment_den); }
        // withdraw all weth (gas refund)
        require(payment <= profit, 'profit');
        IERC20(weth_store).withdraw(add(payment, gas_fees));
        payable(block.coinbase).transfer(payment);
        payable(executioner).transfer(add(gas_fees, sand));
    }

    function random_variable(uint32[2] memory probability) internal view returns(bool){
        uint time = block.timestamp;
        address coinbase = block.coinbase;
        uint variable = uint(keccak256(abi.encode(time, coinbase)));
        if(variable%probability[0] < probability[1]){ return true; }
        else { return false; }
    }

    // Buy trade
    // Even delta if weth position zero
    // packed format: bytes32(abi.encodePacked(address pair, uint token_out))
    // buy_with_weth, method ***REMOVED***
    function ***REMOVED***(bytes32 package) public payable OnlyExecutioner{
        // decode call data
        address pair = address(bytes20(package));
        uint token_out = uint96(bytes12(package << 160));
        // get address, balance and sand
        address hood_add;
        uint hood_bal;
        uint sand;
        assembly {
            hood_add := address()
            hood_bal := selfbalance()
            sand := callvalue()
        }
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
    function loimsadm(address pair, address token) public OnlyExecutioner{
        // get address, balance and sand
        address hood_address = address(this);
        uint hood_bal = hood_address.balance;
        // derive delta_sand from balance
        uint delta_sand = mul(hood_bal, 10 ** 9);
        // set bought_token_amount
        uint bought_token_amount = IERC20(token).balanceOf(hood_address) - 42;
        // calculate eth_out
        (uint weth_reserve, uint token_reserve) = vvcdhivk(pair, weth_store, token);
        uint eth_out = ubmbrnxp(bought_token_amount, token_reserve, weth_reserve);
        // swap
        IERC20(token).transfer(pair, bought_token_amount);
        if (hood_bal % 2 == 0){
            IUniswapV2Pair(pair).swap(eth_out, 0, address(this),"");
        }
        else{
            IUniswapV2Pair(pair).swap(0, eth_out, address(this),"");
        }
        uint gas_fees = mul(200000, block.basefee);
        uint profits = sub(sub(eth_out, delta_sand), gas_fees);
        if (profits != 0){
            // make payment
            make_payment(profits, gas_fees, hood_bal);
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
