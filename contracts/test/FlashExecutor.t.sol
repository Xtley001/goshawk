// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

import "forge-std/Test.sol";
import "../src/FlashExecutor.sol";

contract FlashExecutorTest is Test {
    FlashExecutor executor;

    // Ethereum Mainnet Tokens (06_ORACLES.md §6.1)
    address constant USDC = 0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48;
    address constant WETH = 0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2;

    address executorEOA;
    address coldWallet;

    function setUp() public {
        executorEOA = makeAddr("executor");
        coldWallet  = makeAddr("coldWallet");
        executor = new FlashExecutor(address(this), executorEOA, coldWallet);
    }

    /// @notice Verify verified Ethereum mainnet constants match 05 & 07 specifications.
    function testCanonicalAddresses() public view {
        assertEq(executor.MORPHO_BLUE(), 0xBBBBBbbBBb9cC5e90e3b3Af64bdAF62C37EEFFCb, "Morpho Blue address mismatch");
        assertEq(executor.BALANCER_VAULT(), 0xBA12222222228d8Ba53be47888D16304ca09907c, "Balancer Vault address mismatch");
        assertEq(executor.SPARK_DSS_FLASH(), 0x60744434d6339a6B27d73d9Eda62b6F66a0a04FA, "Spark DSS Flash address mismatch");
        assertEq(executor.AAVE_V3_POOL(), 0x794a61358D6845594F94dc1DB02A252b5b4814aD, "Aave V3 Pool address mismatch");
    }

    /// @notice Verify every gapped protocol constant is strictly address(0) per 08_EXECUTION_CONTRACT.md §8.2.
    function testGappedAddressesAreZero() public view {
        assertEq(executor.SPARK_POOL(), address(0), "SPARK_POOL must be address(0)");
        assertEq(executor.FLUID_LIQUIDITY(), address(0), "FLUID_LIQUIDITY must be address(0)");
        assertEq(executor.COMPOUND_V3_COMET(), address(0), "COMPOUND_V3_COMET must be address(0)");
        assertEq(executor.EULER_V2_VAULT(), address(0), "EULER_V2_VAULT must be address(0)");
        assertEq(executor.UNISWAP_V3_POOL(), address(0), "UNISWAP_V3_POOL must be address(0)");
    }

    /// @notice Verify default whitelists for verified protocols and flash providers.
    function testInitialWhitelists() public view {
        assertTrue(executor.allowedFlashVaults(executor.MORPHO_BLUE()), "Morpho Blue must be whitelisted flash vault");
        assertTrue(executor.allowedFlashVaults(executor.BALANCER_VAULT()), "Balancer Vault must be whitelisted flash vault");
        assertTrue(executor.allowedFlashVaults(executor.SPARK_DSS_FLASH()), "Spark DSS Flash must be whitelisted flash vault");
        assertTrue(executor.allowedFlashVaults(executor.AAVE_V3_POOL()), "Aave V3 Pool must be whitelisted flash vault");

        assertTrue(executor.allowedProtocols(executor.AAVE_V3_POOL()), "Aave V3 Pool must be whitelisted protocol");
        assertTrue(executor.allowedProtocols(executor.MORPHO_BLUE()), "Morpho Blue must be whitelisted protocol");
    }

    /// @notice Calling with an unknown or unwhitelisted protocol reverts UnknownProtocol.
    function testUnknownProtocolReverts() public {
        address unknownProto = makeAddr("unknownProtocol");
        vm.expectRevert(ExecutorBase.UnknownProtocol.selector);
        executor.executeLiquidation(
            executor.MORPHO_BLUE(),
            unknownProto,
            WETH,
            USDC,
            makeAddr("borrower"),
            10_000e6,
            5 ether,
            address(0),
            hex""
        );
    }

    /// @notice Calling with an invalid flash provider reverts InsufficientFlashLiquidity.
    function testInvalidFlashProviderReverts() public {
        address unknownProvider = makeAddr("unknownProvider");
        vm.expectRevert(ExecutorBase.InsufficientFlashLiquidity.selector);
        executor.executeLiquidation(
            unknownProvider,
            executor.AAVE_V3_POOL(),
            WETH,
            USDC,
            makeAddr("borrower"),
            10_000e6,
            5 ether,
            address(0),
            hex""
        );
    }

    /// @notice Non-authorized caller cannot execute liquidation.
    function testOnlyAuthorizedGuard() public {
        address unauth = makeAddr("unauthorized");
        vm.prank(unauth);
        vm.expectRevert(ExecutorBase.Unauthorized.selector);
        executor.executeLiquidation(
            executor.MORPHO_BLUE(),
            executor.AAVE_V3_POOL(),
            WETH,
            USDC,
            makeAddr("borrower"),
            10_000e6,
            5 ether,
            address(0),
            hex""
        );
    }

    /// @notice Pause blocks execution.
    function testPauseBlocksExecution() public {
        executor.setPaused(true);
        assertTrue(executor.paused());

        vm.expectRevert(ExecutorBase.ContractPaused.selector);
        executor.executeLiquidation(
            executor.MORPHO_BLUE(),
            executor.AAVE_V3_POOL(),
            WETH,
            USDC,
            makeAddr("borrower"),
            10_000e6,
            5 ether,
            address(0),
            hex""
        );
    }

    /// @notice Only owner can call sweep().
    function testSweepOnlyOwner() public {
        deal(USDC, address(executor), 5_000e6);
        address recipient = makeAddr("recipient");

        // Non-owner reverts
        vm.prank(executorEOA);
        vm.expectRevert(ExecutorBase.Unauthorized.selector);
        executor.sweep(USDC, recipient);

        // Owner succeeds
        executor.sweep(USDC, recipient);
        assertEq(IERC20(USDC).balanceOf(recipient), 5_000e6);
        assertEq(IERC20(USDC).balanceOf(address(executor)), 0);
    }

    /// @notice Only executor can call emergencySweep() which routes to coldWallet.
    function testEmergencySweepOnlyExecutor() public {
        deal(WETH, address(executor), 2 ether);

        // Owner cannot call emergencySweep
        vm.expectRevert(ExecutorBase.NotExecutor.selector);
        executor.emergencySweep(WETH);

        // Executor succeeds and funds go to coldWallet
        vm.prank(executorEOA);
        executor.emergencySweep(WETH);
        assertEq(IERC20(WETH).balanceOf(coldWallet), 2 ether);
        assertEq(IERC20(WETH).balanceOf(address(executor)), 0);
    }

    /// @notice Callback entrypoints revert if not in active flash loan.
    function testCallbacksRevertWhenNotInFlashLoan() public {
        vm.expectRevert(ExecutorBase.NotInFlashLoan.selector);
        executor.onMorphoFlashLoan(100, hex"");

        vm.expectRevert(ExecutorBase.NotInFlashLoan.selector);
        executor.executeOperation(WETH, 100, 1, address(this), hex"");

        vm.expectRevert(ExecutorBase.NotInFlashLoan.selector);
        executor.onFlashLoan(address(this), WETH, 100, 0, hex"");

        vm.expectRevert(ExecutorBase.NotInFlashLoan.selector);
        IERC20[] memory tokens = new IERC20[](0);
        uint256[] memory amounts = new uint256[](0);
        executor.receiveFlashLoan(tokens, amounts, amounts, hex"");

        vm.expectRevert(ExecutorBase.NotInFlashLoan.selector);
        executor.uniswapV3FlashCallback(0, 0, hex"");
    }
}
