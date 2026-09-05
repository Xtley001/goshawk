// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

import "./ExecutorBase.sol";
import "./interfaces/IAaveV3.sol";
import "./interfaces/IBalancerVault.sol";
import "./interfaces/IMorphoBlue.sol";
import "./interfaces/ISpark.sol";
import "./interfaces/IFluid.sol";
import "./interfaces/ICompoundV3.sol";
import "./interfaces/IEulerV2.sol";
import "./interfaces/IERC3156.sol";
import "./interfaces/IUniswapV3.sol";
import "./interfaces/ICurve.sol";

/// @title  FlashExecutor — Goshawk Ethereum Mainnet liquidation engine
/// @notice 08_EXECUTION_CONTRACT.md §8.2, 05_PROTOCOLS_AND_ADDRESSES.md, 07_FLASH_LOANS_AND_DEX.md
contract FlashExecutor is ExecutorBase {
    using SafeERC20 for IERC20;

    // ─── Protocol Addresses (Ethereum Mainnet, Chain ID 1) ────────────────
    // 05_PROTOCOLS_AND_ADDRESSES.md & 07_FLASH_LOANS_AND_DEX.md
    address public constant MORPHO_BLUE        = 0xBBBBBbbBBb9cC5e90e3b3Af64bdAF62C37EEFFCb;
    address public constant BALANCER_VAULT     = 0xBA12222222228d8Ba53be47888D16304ca09907c;
    address public constant SPARK_DSS_FLASH    = 0x60744434d6339a6B27d73d9Eda62b6F66a0a04FA;
    address public constant AAVE_V3_POOL       = 0x794a61358D6845594F94dc1DB02A252b5b4814aD;

    // Gaps from 05/07 — strictly set to address(0) until sourced per 08 §8.2
    address public constant UNISWAP_V3_POOL    = address(0); // TODO(GAP)
    address public constant SPARK_POOL         = address(0); // TODO(GAP)
    address public constant FLUID_LIQUIDITY    = address(0); // TODO(GAP)
    address public constant COMPOUND_V3_COMET  = address(0); // TODO(GAP)
    address public constant EULER_V2_VAULT     = address(0); // TODO(GAP)

    enum FlashProvider {
        MORPHO_BLUE,       // 0 (0.00%)
        BALANCER_V2,       // 1 (0.00%)
        SPARK_DSS_FLASH,   // 2 (0.00%)
        AAVE_V3,           // 3 (0.05%)
        UNISWAP_V3_FLASH   // 4 (0.05-0.30%)
    }

    enum StrategyType {
        CROSS_DEX_ARB,   // 0
        TRI_ARB,         // 1
        LIQUIDATION      // 2
    }

    struct ExecuteParams {
        FlashProvider provider;
        address[]     tokens;
        uint256[]     amounts;
        StrategyType  stratType;
        bytes         stratData;
        uint256       minProfit;
        address       profitToken;
    }

    // Context tracking during active flash execution
    address private _currentFlashProvider;
    address private _currentDebtAsset;
    uint256 private _currentDebtAmount;

    constructor(
        address _owner,
        address _executor,
        address _coldWallet
    ) ExecutorBase(_owner, _executor, _coldWallet) {
        // Chain ID guard — allows Ethereum Mainnet (1) or local test forks (31337)
        require(
            block.chainid == 1 || block.chainid == 31337,
            "Goshawk: wrong chain -- expected Ethereum Mainnet (1)"
        );

        allowedFlashVaults[MORPHO_BLUE]     = true;
        allowedFlashVaults[BALANCER_VAULT]  = true;
        allowedFlashVaults[SPARK_DSS_FLASH] = true;
        allowedFlashVaults[AAVE_V3_POOL]    = true;

        allowedProtocols[AAVE_V3_POOL] = true;
        allowedProtocols[MORPHO_BLUE]  = true;
    }

    // ─── Primary Entrypoint (08_EXECUTION_CONTRACT.md §8.2) ───────────────
    function executeLiquidation(
        address flashProvider,
        address protocol,
        address collateralAsset,
        address debtAsset,
        address borrower,
        uint256 debtToCover,
        uint256 minCollateralOut,
        address dexRouter,
        bytes calldata dexCalldata
    ) external onlyAuthorized whenNotPaused nonReentrant {
        if (!allowedProtocols[protocol]) revert UnknownProtocol();
        if (dexRouter != address(0) && !allowedRouters[dexRouter]) {
            // If routers are whitelisted, ensure router is valid
            if (allowedRouters[dexRouter] == false && address(dexRouter).code.length == 0) {
                revert SwapFailed();
            }
        }

        bytes memory data = abi.encode(
            protocol,
            collateralAsset,
            borrower,
            minCollateralOut,
            dexRouter,
            dexCalldata
        );

        _inFlashLoan = true;
        _currentFlashProvider = flashProvider;
        _currentDebtAsset = debtAsset;
        _currentDebtAmount = debtToCover;

        _initiateFlashLoan(flashProvider, debtAsset, debtToCover, data);

        _inFlashLoan = false;
        _currentFlashProvider = address(0);
        _currentDebtAsset = address(0);
        _currentDebtAmount = 0;

        _sweepProfitToOwner(debtAsset);
    }

    // ─── Backward-compatible entrypoint ───────────────────────────────────
    function execute(ExecuteParams calldata p) external onlyAuthorized whenNotPaused nonReentrant {
        require(p.tokens.length > 0 && p.amounts.length > 0, "Goshawk: empty tokens/amounts");
        address flashProviderAddr;
        if (p.provider == FlashProvider.MORPHO_BLUE) {
            flashProviderAddr = MORPHO_BLUE;
        } else if (p.provider == FlashProvider.BALANCER_V2) {
            flashProviderAddr = BALANCER_VAULT;
        } else if (p.provider == FlashProvider.SPARK_DSS_FLASH) {
            flashProviderAddr = SPARK_DSS_FLASH;
        } else if (p.provider == FlashProvider.AAVE_V3) {
            flashProviderAddr = AAVE_V3_POOL;
        } else {
            flashProviderAddr = UNISWAP_V3_POOL;
        }

        _inFlashLoan = true;
        _currentFlashProvider = flashProviderAddr;
        _currentDebtAsset = p.tokens[0];
        _currentDebtAmount = p.amounts[0];

        _initiateFlashLoan(flashProviderAddr, p.tokens[0], p.amounts[0], p.stratData);

        _inFlashLoan = false;
        _currentFlashProvider = address(0);
        _currentDebtAsset = address(0);
        _currentDebtAmount = 0;

        uint256 profitBal = IERC20(p.profitToken).balanceOf(address(this));
        if (profitBal < p.minProfit) revert InsufficientProfit();
        _sweepProfitToOwner(p.profitToken);
        emit Executed(profitBal);
    }

    // ─── Flash-Loan Routing (07 §7.2, 08 §8.2) ────────────────────────────
    function _initiateFlashLoan(
        address provider,
        address asset,
        uint256 amount,
        bytes memory data
    ) internal {
        if (provider == MORPHO_BLUE) {
            _borrowMorpho(asset, amount, data);
            return;
        }
        if (provider == BALANCER_VAULT) {
            _borrowBalancer(asset, amount, data);
            return;
        }
        if (provider == SPARK_DSS_FLASH) {
            _borrowSparkDss(asset, amount, data);
            return;
        }
        if (provider == AAVE_V3_POOL) {
            _borrowAaveV3(asset, amount, data);
            return;
        }
        if (provider == UNISWAP_V3_POOL && UNISWAP_V3_POOL != address(0)) {
            _borrowUniV3Flash(asset, amount, data);
            return;
        }
        revert InsufficientFlashLiquidity();
    }

    function _borrowMorpho(address asset, uint256 amount, bytes memory data) internal {
        IMorphoBlue(MORPHO_BLUE).flashLoan(asset, amount, data);
    }

    function _borrowBalancer(address asset, uint256 amount, bytes memory data) internal {
        address[] memory tokens = new address[](1);
        tokens[0] = asset;
        uint256[] memory amounts = new uint256[](1);
        amounts[0] = amount;
        IBalancerVault(BALANCER_VAULT).flashLoan(address(this), tokens, amounts, data);
    }

    function _borrowSparkDss(address asset, uint256 amount, bytes memory data) internal {
        IERC3156FlashLender(SPARK_DSS_FLASH).flashLoan(
            IERC3156FlashBorrower(address(this)),
            asset,
            amount,
            data
        );
    }

    function _borrowAaveV3(address asset, uint256 amount, bytes memory data) internal {
        IAaveV3Pool(AAVE_V3_POOL).flashLoanSimple(address(this), asset, amount, data, 0);
    }

    function _borrowUniV3Flash(address /* asset */, uint256 /* amount */, bytes memory /* data */) internal pure {
        revert InsufficientFlashLiquidity();
    }

    // ─── Flash Loan Callbacks (07 §7.1, 08 §8.2) ──────────────────────────
    function onMorphoFlashLoan(uint256 assets, bytes calldata data) external onlyFlashLoan {
        require(msg.sender == MORPHO_BLUE, "Goshawk: untrusted flash caller");
        _onFlashReceived(assets, 0, data);
        // Morpho Blue pulls repayment via transferFrom
        _ensureApprove(_currentDebtAsset, MORPHO_BLUE, assets);
    }

    function receiveFlashLoan(
        IERC20[] calldata tokens,
        uint256[] calldata amounts,
        uint256[] calldata feeAmounts,
        bytes calldata userData
    ) external onlyFlashLoan {
        require(msg.sender == BALANCER_VAULT, "Goshawk: untrusted flash caller");
        _onFlashReceived(amounts[0], feeAmounts[0], userData);
        // Balancer requires recipient to push repayment back
        tokens[0].safeTransfer(BALANCER_VAULT, amounts[0] + feeAmounts[0]);
    }

    function onFlashLoan(
        address initiator,
        address token,
        uint256 amount,
        uint256 fee,
        bytes calldata data
    ) external onlyFlashLoan returns (bytes32) {
        require(msg.sender == SPARK_DSS_FLASH, "Goshawk: untrusted flash caller");
        require(initiator == address(this), "Goshawk: untrusted flash initiator");
        _onFlashReceived(amount, fee, data);
        // ERC-3156 pulls repayment via transferFrom
        _ensureApprove(token, SPARK_DSS_FLASH, amount + fee);
        return keccak256("ERC3156FlashBorrower.onFlashLoan");
    }

    function executeOperation(
        address asset,
        uint256 amount,
        uint256 premium,
        address initiator,
        bytes calldata params
    ) external onlyFlashLoan returns (bool) {
        require(msg.sender == AAVE_V3_POOL, "Goshawk: untrusted flash caller");
        require(initiator == address(this), "Goshawk: untrusted flash initiator");
        _onFlashReceived(amount, premium, params);
        // Aave V3 pulls repayment via transferFrom
        _ensureApprove(asset, AAVE_V3_POOL, amount + premium);
        return true;
    }

    function uniswapV3FlashCallback(
        uint256 fee0,
        uint256 fee1,
        bytes calldata data
    ) external onlyFlashLoan {
        _onFlashReceived(0, fee0 + fee1, data);
    }

    // ─── Liquidation Dispatcher (05, 08 §8.2) ─────────────────────────────
    function _liquidate(
        address protocol,
        address collateralAsset,
        address debtAsset,
        address borrower,
        uint256 debtToCover
    ) internal returns (uint256 seized) {
        if (protocol == AAVE_V3_POOL) {
            _ensureApprove(debtAsset, AAVE_V3_POOL, debtToCover);
            IAaveV3Pool(AAVE_V3_POOL).liquidationCall(
                collateralAsset,
                debtAsset,
                borrower,
                debtToCover,
                false
            );
        } else if (protocol == MORPHO_BLUE) {
            _ensureApprove(debtAsset, MORPHO_BLUE, debtToCover);
            // Off-chain adapter provides calldata or executes liquidation
        } else if (protocol == SPARK_POOL) {
            if (SPARK_POOL == address(0)) revert UnknownProtocol();
            _ensureApprove(debtAsset, SPARK_POOL, debtToCover);
            ISparkPool(SPARK_POOL).liquidationCall(
                collateralAsset,
                debtAsset,
                borrower,
                debtToCover,
                false
            );
        } else if (protocol == FLUID_LIQUIDITY) {
            if (FLUID_LIQUIDITY == address(0)) revert UnknownProtocol();
            _ensureApprove(debtAsset, FLUID_LIQUIDITY, debtToCover);
            IFluid(FLUID_LIQUIDITY).liquidate(collateralAsset, debtAsset, borrower, debtToCover);
        } else if (protocol == COMPOUND_V3_COMET) {
            if (COMPOUND_V3_COMET == address(0)) revert UnknownProtocol();
            address[] memory accts = new address[](1);
            accts[0] = borrower;
            ICompoundV3(COMPOUND_V3_COMET).absorb(address(this), accts);
        } else if (protocol == EULER_V2_VAULT) {
            if (EULER_V2_VAULT == address(0)) revert UnknownProtocol();
            _ensureApprove(debtAsset, EULER_V2_VAULT, debtToCover);
            IEulerV2(EULER_V2_VAULT).liquidateVault(
                collateralAsset,
                debtAsset,
                borrower,
                debtToCover
            );
        } else {
            revert UnknownProtocol();
        }

        seized = IERC20(collateralAsset).balanceOf(address(this));
    }

    // ─── Execution Pipeline (08_EXECUTION_CONTRACT.md §8.2) ───────────────
    function _onFlashReceived(uint256 amount, uint256 fee, bytes memory data) internal {
        if (data.length == 0) return;

        (
            address protocol,
            address collateralAsset,
            address borrower,
            uint256 minCollateralOut,
            address dexRouter,
            bytes memory dexCalldata
        ) = abi.decode(data, (address, address, address, uint256, address, bytes));

        address debtAsset = _currentDebtAsset;
        uint256 seized = _liquidate(protocol, collateralAsset, debtAsset, borrower, amount);

        if (dexRouter != address(0) && dexCalldata.length > 0) {
            _ensureApprove(collateralAsset, dexRouter, seized);
            (bool ok, bytes memory result) = dexRouter.call(dexCalldata);
            if (!ok) revert SwapFailed();
            if (result.length >= 32) {
                uint256 debtOut = abi.decode(result, (uint256));
                if (debtOut < minCollateralOut) revert SlippageExceeded();
            }
        }

        uint256 currentDebtBal = IERC20(debtAsset).balanceOf(address(this));
        require(currentDebtBal >= amount + fee, "Goshawk: insufficient funds to repay flash loan");
    }
}
