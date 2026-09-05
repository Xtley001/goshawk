// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

import "@openzeppelin/contracts/token/ERC20/IERC20.sol";
import "@openzeppelin/contracts/token/ERC20/utils/SafeERC20.sol";

/// @title  ExecutorBase — Goshawk base liquidation and execution control
/// @notice Ethereum Mainnet (Chain ID 1) — 08_EXECUTION_CONTRACT.md
abstract contract ExecutorBase {
    using SafeERC20 for IERC20;

    // ─── Custom Errors ───────────────────────────────────────────────────
    error Unauthorized();
    error NotExecutor();
    error ContractPaused();
    error ReentrantCall();
    error NotInFlashLoan();
    error InsufficientFlashLiquidity();
    error UnknownProtocol();
    error SwapFailed();
    error SlippageExceeded();
    error ZeroAddress();
    error TimelockNotElapsed();
    error NoPendingExecutor();
    error InsufficientProfit();

    // ─── Reentrancy Guard ────────────────────────────────────────────────
    uint256 private constant _NOT_ENTERED = 1;
    uint256 private constant _ENTERED     = 2;
    uint256 private _status               = _NOT_ENTERED;

    modifier nonReentrant() {
        if (_status == _ENTERED) revert ReentrantCall();
        _status = _ENTERED;
        _;
        _status = _NOT_ENTERED;
    }

    bool internal _inFlashLoan;
    modifier onlyFlashLoan() {
        if (!_inFlashLoan) revert NotInFlashLoan();
        _;
    }

    // ─── Emergency Pause ─────────────────────────────────────────────────
    bool public paused;
    modifier whenNotPaused() {
        if (paused) revert ContractPaused();
        _;
    }

    function setPaused(bool _paused) external onlyOwner {
        paused = _paused;
        emit Paused(_paused);
    }

    // ─── Access Control ──────────────────────────────────────────────────
    address public immutable owner;
    address public executor;
    address public immutable coldWallet;

    address public pendingExecutor;
    uint256 public executorChangeETA;

    modifier onlyOwner() {
        if (msg.sender != owner) revert Unauthorized();
        _;
    }

    modifier onlyExecutor() {
        if (msg.sender != executor) revert NotExecutor();
        _;
    }

    modifier onlyAuthorized() {
        if (msg.sender != owner && msg.sender != executor) revert Unauthorized();
        _;
    }

    // ─── Whitelisted protocols & routers ─────────────────────────────────
    mapping(address => bool) public allowedProtocols;
    mapping(address => bool) public allowedRouters;
    mapping(address => bool) public allowedFlashVaults;

    // ─── Events ──────────────────────────────────────────────────────────
    event Executed(uint256 profit);
    event ProtocolWhitelisted(address indexed proto, bool allowed);
    event RouterWhitelisted(address indexed router, bool allowed);
    event FlashVaultWhitelisted(address indexed vault, bool allowed);
    event Paused(bool indexed isPaused);
    event ExecutorProposed(address indexed proposed, uint256 eta);
    event ExecutorAccepted(address indexed newExecutor);

    constructor(address _owner, address _executor, address _coldWallet) {
        if (_owner == address(0) || _executor == address(0) || _coldWallet == address(0)) {
            revert ZeroAddress();
        }
        owner = _owner;
        executor = _executor;
        coldWallet = _coldWallet;
    }

    function proposeExecutor(address _new) external onlyOwner {
        if (_new == address(0)) revert ZeroAddress();
        pendingExecutor = _new;
        executorChangeETA = block.timestamp + 24 hours;
        emit ExecutorProposed(_new, executorChangeETA);
    }

    function acceptExecutor() external onlyOwner {
        if (pendingExecutor == address(0)) revert NoPendingExecutor();
        if (block.timestamp < executorChangeETA) revert TimelockNotElapsed();
        executor = pendingExecutor;
        pendingExecutor = address(0);
        executorChangeETA = 0;
        emit ExecutorAccepted(executor);
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

    // ─── Sweeping Functions ──────────────────────────────────────────────
    function sweep(address token, address to) external onlyOwner {
        if (to == address(0)) revert ZeroAddress();
        uint256 bal = IERC20(token).balanceOf(address(this));
        if (bal > 0) {
            IERC20(token).safeTransfer(to, bal);
        }
    }

    function emergencySweep(address token) external onlyExecutor {
        uint256 bal = IERC20(token).balanceOf(address(this));
        if (bal > 0) {
            IERC20(token).safeTransfer(coldWallet, bal);
        }
    }

    function _sweepProfitToOwner(address token) internal {
        uint256 bal = IERC20(token).balanceOf(address(this));
        if (bal > 0) {
            IERC20(token).safeTransfer(owner, bal);
        }
    }

    function _ensureApprove(address token, address spender, uint256 amount) internal {
        if (token == address(0) || spender == address(0)) return;
        uint256 currentAllowance = IERC20(token).allowance(address(this), spender);
        if (currentAllowance < amount) {
            if (currentAllowance > 0) {
                IERC20(token).safeApprove(spender, 0);
            }
            IERC20(token).safeApprove(spender, type(uint256).max);
        }
    }
}
