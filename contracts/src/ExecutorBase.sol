// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

import "./interfaces/IBalancerVault.sol";
import "./interfaces/IMorphoBlue.sol";
import "./interfaces/IAaveV3.sol";
import "./interfaces/IHyperLend.sol";
import "./interfaces/ISpark.sol";
import "@openzeppelin/contracts/token/ERC20/IERC20.sol";
import "@openzeppelin/contracts/token/ERC20/utils/SafeERC20.sol";

/// @title  ExecutorBase — Unified liquidation engine with 4 flash callback branches
/// @notice Deployed identically across all 10 chains per 10_CONTRACTS.md
contract ExecutorBase {
    using SafeERC20 for IERC20;

    // ─── Reentrancy guard ────────────────────────────────────────────────
    uint256 private constant _NOT_ENTERED = 1;
    uint256 private constant _ENTERED     = 2;
    uint256 private _status               = _NOT_ENTERED;

    modifier nonReentrant() {
        require(_status != _ENTERED, "Corvus: reentrant call");
        _status = _ENTERED;
        _;
        _status = _NOT_ENTERED;
    }

    bool private _inFlashLoan;
    modifier onlyFlashLoan() {
        require(_inFlashLoan, "Corvus: not in flash loan");
        _;
    }

    // ─── Emergency pause ──────────────────────────────────────────────────
    bool public paused;
    modifier whenNotPaused() { require(!paused, "Corvus: paused"); _; }
    function setPaused(bool _paused) external onlyOwner { paused = _paused; emit Paused(_paused); }

    // ─── Access control ───────────────────────────────────────────────────
    address public immutable owner;
    address public executor;
    address public immutable coldWallet;

    address public pendingExecutor;
    uint256 public executorChangeETA;

    // ─── Whitelisted protocols & routers ─────────────────────────────────
    mapping(address => bool) public allowedProtocols;
    mapping(address => bool) public allowedRouters;
    mapping(address => bool) public allowedFlashVaults;

    // ─── Protocol addresses (configurable per chain deployment) ──────────
    address public balancerVault;
    address public morphoBlue;
    address public aavePool;
    address public hyperLendPool;
    address public sparkPool;

    // ─── Events ───────────────────────────────────────────────────────────
    event Executed(uint256 profit);
    event ProtocolWhitelisted(address indexed proto, bool allowed);
    event RouterWhitelisted(address indexed router, bool allowed);
    event FlashVaultWhitelisted(address indexed vault, bool allowed);
    event Paused(bool indexed isPaused);
    event ExecutorProposed(address indexed proposed, uint256 eta);
    event ExecutorAccepted(address indexed newExecutor);

    enum FlashProvider {
        BALANCER,   // 0
        MORPHO,     // 1
        AAVE,       // 2
        HYPERLEND   // 3
    }

    struct ExecuteParams {
        FlashProvider provider;
        address[]     tokens;
        uint256[]     amounts;
        uint8         stratType;   // 2 = LIQUIDATION
        bytes         stratData;
        uint256       minProfit;
        address       profitToken;
    }

    modifier onlyExecutor() { require(msg.sender == executor, "Corvus: not executor"); _; }
    modifier onlyOwner()    { require(msg.sender == owner,    "Corvus: not owner");    _; }

    constructor(
        address _owner,
        address _executor,
        address _coldWallet,
        address _balancerVault,
        address _morphoBlue,
        address _aavePool,
        address _hyperLendPool
    ) {
        require(_owner      != address(0), "Corvus: zero owner");
        require(_executor   != address(0), "Corvus: zero executor");
        require(_coldWallet != address(0), "Corvus: zero cold wallet");

        owner       = _owner;
        executor    = _executor;
        coldWallet  = _coldWallet;

        balancerVault = _balancerVault;
        morphoBlue    = _morphoBlue;
        aavePool      = _aavePool;
        hyperLendPool = _hyperLendPool;

        if (_balancerVault != address(0)) { allowedFlashVaults[_balancerVault] = true; }
        if (_morphoBlue    != address(0)) { allowedFlashVaults[_morphoBlue]    = true; }
        if (_aavePool      != address(0)) { allowedFlashVaults[_aavePool]      = true; }
        if (_hyperLendPool != address(0)) { allowedFlashVaults[_hyperLendPool] = true; }
    }

    function setProtocols(address _balancer, address _morpho, address _aave, address _hyperlend, address _spark) external onlyOwner {
        balancerVault = _balancer;
        morphoBlue    = _morpho;
        aavePool      = _aave;
        hyperLendPool = _hyperlend;
        sparkPool     = _spark;

        if (_balancer  != address(0)) allowedFlashVaults[_balancer]  = true;
        if (_morpho    != address(0)) allowedFlashVaults[_morpho]    = true;
        if (_aave      != address(0)) allowedFlashVaults[_aave]      = true;
        if (_hyperlend != address(0)) allowedFlashVaults[_hyperlend] = true;
    }

    function setProtocolWhitelisted(address proto, bool allowed) external onlyOwner {
        allowedProtocols[proto] = allowed;
        emit ProtocolWhitelisted(proto, allowed);
    }

    function setRouterWhitelisted(address router, bool allowed) external onlyOwner {
        allowedRouters[router] = allowed;
        emit RouterWhitelisted(router, allowed);
    }

    function setFlashVaultWhitelisted(address vault, bool allowed) external onlyOwner {
        allowedFlashVaults[vault] = allowed;
        emit FlashVaultWhitelisted(vault, allowed);
    }

    // ─── Two-step executor transfer ───────────────────────────────────────
    function proposeExecutor(address _newExecutor) external onlyOwner {
        require(_newExecutor != address(0), "Corvus: zero executor");
        pendingExecutor = _newExecutor;
        executorChangeETA = block.timestamp + 24 hours;
        emit ExecutorProposed(_newExecutor, executorChangeETA);
    }

    function acceptExecutor() external {
        require(msg.sender == pendingExecutor, "Corvus: not pending executor");
        require(block.timestamp >= executorChangeETA, "Corvus: timelock not expired");
        executor = pendingExecutor;
        pendingExecutor = address(0);
        executorChangeETA = 0;
        emit ExecutorAccepted(executor);
    }

    // ─── Flash Loan Execution Entrypoint ──────────────────────────────────
    function execute(ExecuteParams calldata params) external onlyExecutor nonReentrant whenNotPaused {
        require(params.tokens.length > 0, "Corvus: no tokens");
        require(params.amounts.length == params.tokens.length, "Corvus: length mismatch");

        bytes memory callbackData = abi.encode(
            params.stratData,
            params.minProfit,
            params.profitToken
        );

        _inFlashLoan = true;

        if (params.provider == FlashProvider.BALANCER) {
            require(balancerVault != address(0), "Corvus: balancer not configured");
            IBalancerVault(balancerVault).flashLoan(
                address(this),
                params.tokens,
                params.amounts,
                callbackData
            );
        } else if (params.provider == FlashProvider.MORPHO) {
            require(morphoBlue != address(0), "Corvus: morpho not configured");
            require(params.tokens.length == 1, "Corvus: morpho supports 1 asset");
            IMorphoBlue(morphoBlue).flashLoan(
                params.tokens[0],
                params.amounts[0],
                callbackData
            );
        } else if (params.provider == FlashProvider.AAVE) {
            require(aavePool != address(0), "Corvus: aave not configured");
            require(params.tokens.length == 1, "Corvus: aave simple supports 1 asset");
            IAaveV3Pool(aavePool).flashLoanSimple(
                address(this),
                params.tokens[0],
                params.amounts[0],
                callbackData,
                0
            );
        } else if (params.provider == FlashProvider.HYPERLEND) {
            require(hyperLendPool != address(0), "Corvus: hyperlend not configured");
            uint256[] memory modes = new uint256[](params.tokens.length);
            IHyperLendPool(hyperLendPool).flashLoan(
                address(this),
                params.tokens,
                params.amounts,
                modes,
                address(this),
                callbackData,
                0
            );
        }

        _inFlashLoan = false;
    }

    // ─── Branch 1: Balancer V2/V3 Flash Callback ──────────────────────────
    function receiveFlashLoan(
        IERC20[] memory tokens,
        uint256[] memory amounts,
        uint256[] memory feeAmounts,
        bytes memory userData
    ) external onlyFlashLoan {
        require(allowedFlashVaults[msg.sender] || msg.sender == balancerVault, "Corvus: unauthorized vault");

        (bytes memory stratData, uint256 minProfit, address profitToken) =
            abi.decode(userData, (bytes, uint256, address));

        uint256 profit = _executeLiquidationInternal(
            address(tokens[0]),
            amounts[0],
            feeAmounts[0],
            stratData,
            minProfit,
            profitToken
        );

        // Repay Balancer
        tokens[0].safeTransfer(msg.sender, amounts[0] + feeAmounts[0]);
        emit Executed(profit);
    }

    // ─── Branch 2: Aave V3 Single-Asset Flash Callback ────────────────────
    function executeOperation(
        address asset,
        uint256 amount,
        uint256 premium,
        address initiator,
        bytes calldata params
    ) external onlyFlashLoan returns (bool) {
        require(allowedFlashVaults[msg.sender] || msg.sender == aavePool, "Corvus: unauthorized aave pool");
        require(initiator == address(this), "Corvus: invalid initiator");

        (bytes memory stratData, uint256 minProfit, address profitToken) =
            abi.decode(params, (bytes, uint256, address));

        uint256 profit = _executeLiquidationInternal(
            asset,
            amount,
            premium,
            stratData,
            minProfit,
            profitToken
        );

        // Approve Aave to pull repayment
        IERC20(asset).forceApprove(msg.sender, amount + premium);
        emit Executed(profit);
        return true;
    }

    // ─── Branch 3: Morpho Blue Flash Callback ─────────────────────────────
    function onMorphoFlashLoan(
        uint256 assets,
        bytes calldata data
    ) external onlyFlashLoan {
        require(allowedFlashVaults[msg.sender] || msg.sender == morphoBlue, "Corvus: unauthorized morpho");

        (bytes memory stratData, uint256 minProfit, address profitToken) =
            abi.decode(data, (bytes, uint256, address));

        // Note: Morpho passes token in stratData/liquidation context
        (, , , address debtAsset, , , , ) = abi.decode(stratData, (uint256, address, address, address, uint256, bytes, address, bytes));

        uint256 profit = _executeLiquidationInternal(
            debtAsset,
            assets,
            0, // Morpho flash fee is 0
            stratData,
            minProfit,
            profitToken
        );

        // Approve Morpho to pull repayment
        IERC20(debtAsset).forceApprove(msg.sender, assets);
        emit Executed(profit);
    }

    // ─── Branch 4: HyperLend Native / Multi-Asset Callback ────────────────
    function executeOperation(
        address[] calldata assets,
        uint256[] calldata amounts,
        uint256[] calldata premiums,
        address initiator,
        bytes calldata params
    ) external onlyFlashLoan returns (bool) {
        require(allowedFlashVaults[msg.sender] || msg.sender == hyperLendPool, "Corvus: unauthorized hyperlend pool");
        require(initiator == address(this), "Corvus: invalid initiator");

        (bytes memory stratData, uint256 minProfit, address profitToken) =
            abi.decode(params, (bytes, uint256, address));

        uint256 profit = _executeLiquidationInternal(
            assets[0],
            amounts[0],
            premiums[0],
            stratData,
            minProfit,
            profitToken
        );

        // Approve HyperLend to pull repayment
        IERC20(assets[0]).forceApprove(msg.sender, amounts[0] + premiums[0]);
        emit Executed(profit);
        return true;
    }

    // ─── Shared Liquidation Core Sequence ─────────────────────────────────
    function _executeLiquidationInternal(
        address borrowedToken,
        uint256 borrowedAmount,
        uint256 feeAmount,
        bytes memory stratData,
        uint256 minProfit,
        address profitToken
    ) internal returns (uint256 profit) {
        (
            uint256 protocolId,
            address borrower,
            address collateralAsset,
            address debtAsset,
            uint256 debtToCover,
            bytes memory liquidationData,
            address router,
            bytes memory swapRoute
        ) = abi.decode(stratData, (uint256, address, address, address, uint256, bytes, address, bytes));

        require(debtAsset == borrowedToken, "Corvus: debt token mismatch");

        // 1. Execute protocol liquidation call
        if (protocolId == 0) {
            // Morpho Blue
            require(morphoBlue != address(0), "Corvus: morpho not set");
            IERC20(debtAsset).forceApprove(morphoBlue, debtToCover);
            if (liquidationData.length > 0) {
                (bool success, bytes memory ret) = morphoBlue.call(liquidationData);
                require(success, string(ret));
            }
            IERC20(debtAsset).forceApprove(morphoBlue, 0);
        } else if (protocolId == 1) {
            // Aave V3
            require(aavePool != address(0), "Corvus: aave pool not set");
            IERC20(debtAsset).forceApprove(aavePool, debtToCover);
            IAaveV3Pool(aavePool).liquidationCall(
                collateralAsset,
                debtAsset,
                borrower,
                debtToCover,
                false
            );
            IERC20(debtAsset).forceApprove(aavePool, 0);
        } else if (protocolId == 2) {
            // HyperLend
            require(hyperLendPool != address(0), "Corvus: hyperlend pool not set");
            IERC20(debtAsset).forceApprove(hyperLendPool, debtToCover);
            IHyperLendPool(hyperLendPool).liquidationCall(
                collateralAsset,
                debtAsset,
                borrower,
                debtToCover,
                false
            );
            IERC20(debtAsset).forceApprove(hyperLendPool, 0);
        } else if (protocolId == 3) {
            // Spark
            require(sparkPool != address(0), "Corvus: spark pool not set");
            IERC20(debtAsset).forceApprove(sparkPool, debtToCover);
            ISparkPool(sparkPool).liquidationCall(
                collateralAsset,
                debtAsset,
                borrower,
                debtToCover,
                false
            );
            IERC20(debtAsset).forceApprove(sparkPool, 0);
        }

        // 2. Swap seized collateral back to borrowed debt token
        uint256 collateralBal = IERC20(collateralAsset).balanceOf(address(this));
        if (collateralAsset != borrowedToken && collateralBal > 0 && router != address(0)) {
            require(allowedRouters[router], "Corvus: router not whitelisted");
            IERC20(collateralAsset).forceApprove(router, collateralBal);
            if (swapRoute.length > 0) {
                (bool swapSuccess, bytes memory swapRet) = router.call(swapRoute);
                require(swapSuccess, string(swapRet));
            }
            IERC20(collateralAsset).forceApprove(router, 0);
        }

        // 3. Verify flash loan repayment solvency
        uint256 repayAmount = borrowedAmount + feeAmount;
        uint256 debtBal = IERC20(borrowedToken).balanceOf(address(this));
        require(debtBal >= repayAmount, "Corvus: flash repayment insolvent");

        // 4. On-chain profit gate and cold wallet sweep
        address targetProfitToken = profitToken == address(0) ? borrowedToken : profitToken;
        uint256 profitBal = IERC20(targetProfitToken).balanceOf(address(this));
        if (targetProfitToken == borrowedToken) {
            profitBal = profitBal >= repayAmount ? profitBal - repayAmount : 0;
        }

        require(profitBal >= minProfit, "Corvus: profit below min threshold");

        if (profitBal > 0) {
            IERC20(targetProfitToken).safeTransfer(coldWallet, profitBal);
        }

        return profitBal;
    }

    // ─── Emergency token recovery ─────────────────────────────────────────
    function sweepTokens(address token, address recipient) external onlyOwner {
        require(!_inFlashLoan, "Corvus: in flash loan");
        require(recipient != address(0), "Corvus: zero recipient");
        uint256 bal = IERC20(token).balanceOf(address(this));
        if (bal > 0) {
            IERC20(token).safeTransfer(recipient, bal);
        }
    }
}
