// SPDX-License-Identifier: MIT

pragma solidity 0.8.9;

import { SafeTokenTransfer, SafeNativeTransfer } from '@axelar-network/axelar-gmp-sdk-solidity/contracts/utils/SafeTransfer.sol';
import { IERC20 } from '@axelar-network/axelar-gmp-sdk-solidity/contracts/interfaces/IERC20.sol';
import { IAxelarGateway } from '../interfaces/IAxelarGateway.sol';
import { IWETH9 } from '../interfaces/IWETH9.sol';
import { IAxelarDepositService } from '../interfaces/IAxelarDepositService.sol';
import { DepositServiceBase } from './DepositServiceBase.sol';

// This should be owned by the microservice that is paying for gas.
contract ReceiverImplementation is DepositServiceBase {
    using SafeTokenTransfer for IERC20;
    using SafeNativeTransfer for address;

    constructor(address gateway_, string memory wrappedSymbol_) DepositServiceBase(gateway_, wrappedSymbol_) {}

    // @dev This function is used for delegate call by DepositReceiver
    // Context: msg.sender == AxelarDepositService, this == DepositReceiver
    function receiveAndSendToken(
        address refundAddress,
        string calldata destinationChain,
        string calldata destinationAddress,
        string calldata symbol
    ) external {
        address tokenAddress = IAxelarGateway(gateway).tokenAddresses(symbol);
        // Checking with AxelarDepositService if need to refund a token
        address refund = IAxelarDepositService(msg.sender).refundToken();

        if (refund != address(0)) {
            if (refundAddress == address(0)) refundAddress = msg.sender;
            IERC20(refund).safeTransfer(refundAddress, IERC20(refund).balanceOf(address(this)));
            return;
        }

        uint256 amount = IERC20(tokenAddress).balanceOf(address(this));

        if (tokenAddress == address(0)) revert InvalidSymbol();
        if (amount == 0) revert NothingDeposited();

        // Not doing safe approval as gateway will revert anyway if approval fails
        // We expect allowance to always be 0 at this point
        IERC20(tokenAddress).approve(gateway, amount);
        // Sending the token trough the gateway
        IAxelarGateway(gateway).sendToken(destinationChain, destinationAddress, symbol, amount);
    }

    // @dev This function is used for delegate call by DepositReceiver
    // Context: msg.sender == AxelarDepositService, this == DepositReceiver
    function receiveAndSendNative(
        address refundAddress,
        string calldata destinationChain,
        string calldata destinationAddress
    ) external {
        address refund = IAxelarDepositService(msg.sender).refundToken();

        if (refund != address(0)) {
            if (refundAddress == address(0)) refundAddress = msg.sender;
            IERC20(refund).safeTransfer(refundAddress, IERC20(refund).balanceOf(address(this)));
            return;
        }

        address wrappedTokenAddress = wrappedToken();
        uint256 amount = address(this).balance;

        if (wrappedTokenAddress == address(0)) revert WrappedTokenNotSupported();
        if (amount == 0) revert NothingDeposited();

        // Wrapping the native currency and into WETH-like
        IWETH9(wrappedTokenAddress).deposit{ value: amount }();
        // Not doing safe approval as gateway will revert anyway if approval fails
        // We expect allowance to always be 0 at this point
        IWETH9(wrappedTokenAddress).approve(gateway, amount);
        // Sending the token trough the gateway
        IAxelarGateway(gateway).sendToken(destinationChain, destinationAddress, wrappedSymbol(), amount);
    }

    // @dev This function is used for delegate call by DepositReceiver
    // Context: msg.sender == AxelarDepositService, this == DepositReceiver
    function receiveAndUnwrapNative(address refundAddress, address recipient) external {
        address wrappedTokenAddress = wrappedToken();
        address refund = IAxelarDepositService(msg.sender).refundToken();

        if (refund != address(0)) {
            if (refundAddress == address(0)) refundAddress = msg.sender;
            IERC20(refund).safeTransfer(refundAddress, IERC20(refund).balanceOf(address(this)));
            return;
        }

        uint256 amount = IERC20(wrappedTokenAddress).balanceOf(address(this));

        if (wrappedTokenAddress == address(0)) revert WrappedTokenNotSupported();
        if (amount == 0) revert NothingDeposited();

        // Unwrapping the token into native currency and sending it to the recipient
        IWETH9(wrappedTokenAddress).withdraw(amount);
        recipient.safeNativeTransfer(amount);
    }
}
