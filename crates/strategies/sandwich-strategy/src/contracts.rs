use alloy_sol_types::sol;

sol! {
    /// WETH 合约接口
    #[sol(rpc)]
    contract WETH {
        function balanceOf(address account) external view returns (uint256);
        function transfer(address to, uint256 amount) external returns (bool);
        function deposit() external payable;
        function withdraw(uint256 amount) external;
    }

    /// Uniswap V2 Pool 接口
    #[sol(rpc)]
    contract UniswapV2Pool {
        function getReserves() external view returns (uint112 reserve0, uint112 reserve1, uint32 blockTimestampLast);
        function token0() external view returns (address);
        function token1() external view returns (address);
        function swap(uint amount0Out, uint amount1Out, address to, bytes calldata data) external;
    }

    /// Uniswap V3 Pool 接口
    #[sol(rpc)]
    contract UniswapV3Pool {
        function token0() external view returns (address);
        function token1() external view returns (address);
        function fee() external view returns (uint24);
        function slot0() external view returns (
            uint160 sqrtPriceX96,
            int24 tick,
            uint16 observationIndex,
            uint16 observationCardinality,
            uint16 observationCardinalityNext,
            uint8 feeProtocol,
            bool unlocked
        );
        function liquidity() external view returns (uint128);
    }

    /// Sandwich 合约接口（基于 rusty-sando 的 Huff 合约）
    #[sol(rpc)]
    contract SandwichContract {
        /// 执行 V2 Sandwich 攻击
        function sandwichV2(
            address target_pool,
            address intermediary_token,
            uint256 amount_in,
            uint256 amount_out_min,
            bool is_weth_input
        ) external payable;

        /// 执行 V3 Sandwich 攻击
        function sandwichV3(
            address target_pool,
            address intermediary_token,
            uint256 amount_in,
            uint256 amount_out_min,
            uint160 sqrt_price_limit_x96,
            bool zero_for_one
        ) external payable;

        /// 获取合约的 WETH 余额
        function getWethBalance() external view returns (uint256);

        /// 获取合约的代币余额
        function getTokenBalance(address token) external view returns (uint256);

        /// 紧急提取函数
        function emergencyWithdraw(address token, uint256 amount) external;
    }

    /// ERC20 标准接口
    #[sol(rpc)]
    contract ERC20 {
        function balanceOf(address account) external view returns (uint256);
        function transfer(address to, uint256 amount) external returns (bool);
        function transferFrom(address from, address to, uint256 amount) external returns (bool);
        function approve(address spender, uint256 amount) external returns (bool);
        function allowance(address owner, address spender) external view returns (uint256);
        function totalSupply() external view returns (uint256);
        function decimals() external view returns (uint8);
        function symbol() external view returns (string memory);
        function name() external view returns (string memory);
    }

    /// Uniswap V2 Router 接口
    #[sol(rpc)]
    contract UniswapV2Router {
        function swapExactTokensForTokens(
            uint amountIn,
            uint amountOutMin,
            address[] calldata path,
            address to,
            uint deadline
        ) external returns (uint[] memory amounts);

        function swapTokensForExactTokens(
            uint amountOut,
            uint amountInMax,
            address[] calldata path,
            address to,
            uint deadline
        ) external returns (uint[] memory amounts);

        function swapExactETHForTokens(
            uint amountOutMin,
            address[] calldata path,
            address to,
            uint deadline
        ) external payable returns (uint[] memory amounts);

        function swapTokensForExactETH(
            uint amountOut,
            uint amountInMax,
            address[] calldata path,
            address to,
            uint deadline
        ) external returns (uint[] memory amounts);

        function getAmountsOut(uint amountIn, address[] calldata path)
            external view returns (uint[] memory amounts);

        function getAmountsIn(uint amountOut, address[] calldata path)
            external view returns (uint[] memory amounts);
    }

    /// Uniswap V3 Router 接口
    #[sol(rpc)]
    contract UniswapV3Router {
        struct ExactInputSingleParams {
            address tokenIn;
            address tokenOut;
            uint24 fee;
            address recipient;
            uint256 deadline;
            uint256 amountIn;
            uint256 amountOutMinimum;
            uint160 sqrtPriceLimitX96;
        }

        struct ExactOutputSingleParams {
            address tokenIn;
            address tokenOut;
            uint24 fee;
            address recipient;
            uint256 deadline;
            uint256 amountOut;
            uint256 amountInMaximum;
            uint160 sqrtPriceLimitX96;
        }

        function exactInputSingle(ExactInputSingleParams calldata params)
            external payable returns (uint256 amountOut);

        function exactOutputSingle(ExactOutputSingleParams calldata params)
            external payable returns (uint256 amountIn);
    }
}

/// 常用的合约地址常量
pub mod addresses {
    use artemis_core::eth::Address;
    use once_cell::sync::Lazy;

    /// WETH 合约地址
    pub static WETH: Lazy<Address> = Lazy::new(|| {
        "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2".parse().unwrap()
    });

    /// Uniswap V2 Factory
    pub static UNISWAP_V2_FACTORY: Lazy<Address> = Lazy::new(|| {
        "0x5C69bEe701ef814a2B6a3EDD4B1652CB9cc5aA6f".parse().unwrap()
    });

    /// Uniswap V2 Router
    pub static UNISWAP_V2_ROUTER: Lazy<Address> = Lazy::new(|| {
        "0x7a250d5630B4cF539739dF2C5dAcb4c659F2488D".parse().unwrap()
    });

    /// Uniswap V3 Factory
    pub static UNISWAP_V3_FACTORY: Lazy<Address> = Lazy::new(|| {
        "0x1F98431c8aD98523631AE4a59f267346ea31F984".parse().unwrap()
    });

    /// Uniswap V3 Router
    pub static UNISWAP_V3_ROUTER: Lazy<Address> = Lazy::new(|| {
        "0xE592427A0AEce92De3Edee1F18E0157C05861564".parse().unwrap()
    });

    /// Sushiswap Router
    pub static SUSHISWAP_ROUTER: Lazy<Address> = Lazy::new(|| {
        "0xd9e1cE17f2641f24aE83637ab66a2cca9C378B9F".parse().unwrap()
    });

    /// 1inch Router
    pub static ONEINCH_ROUTER: Lazy<Address> = Lazy::new(|| {
        "0x1111111254EEB25477B68fb85Ed929f73A960582".parse().unwrap()
    });
}

/// 函数选择器常量
pub mod selectors {
    /// swapExactTokensForTokens
    pub const SWAP_EXACT_TOKENS_FOR_TOKENS: [u8; 4] = [0x38, 0xed, 0x17, 0x39];
    /// swapTokensForExactTokens
    pub const SWAP_TOKENS_FOR_EXACT_TOKENS: [u8; 4] = [0x88, 0x03, 0xdb, 0xee];
    /// swapExactETHForTokens
    pub const SWAP_EXACT_ETH_FOR_TOKENS: [u8; 4] = [0x7f, 0xf3, 0x6a, 0xb5];
    /// swapTokensForExactETH
    pub const SWAP_TOKENS_FOR_EXACT_ETH: [u8; 4] = [0x47, 0x51, 0xb7, 0xb1];
    /// swapExactTokensForETH
    pub const SWAP_EXACT_TOKENS_FOR_ETH: [u8; 4] = [0x18, 0xcb, 0xaf, 0xe5];
    /// swapETHForExactTokens
    pub const SWAP_ETH_FOR_EXACT_TOKENS: [u8; 4] = [0xfb, 0x3b, 0xdb, 0x41];
    /// Uniswap V3 exactInputSingle
    pub const EXACT_INPUT_SINGLE: [u8; 4] = [0x41, 0x4b, 0xf3, 0x89];
    /// Uniswap V3 exactOutputSingle
    pub const EXACT_OUTPUT_SINGLE: [u8; 4] = [0xdb, 0x3e, 0x21, 0x98];
}
