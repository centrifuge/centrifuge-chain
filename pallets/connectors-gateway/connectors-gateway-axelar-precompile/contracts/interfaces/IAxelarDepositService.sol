// SPDX-License-Identifier: MIT

pragma solidity ^0.8.0;

import { IUpgradable } from './IUpgradable.sol';
import { IDepositServiceBase } from './IDepositServiceBase.sol';

interface IAxelarDepositService is IUpgradable, IDepositServiceBase {
    function sendNative(string calldata destinationChain, string calldata destinationAddress) external payable;

    function addressForTokenDeposit(
        bytes32 salt,
        address refundAddress,
        string calldata destinationChain,
        string calldata destinationAddress,
        string calldata tokenSymbol
    ) external view returns (address);

    function addressForNativeDeposit(
        bytes32 salt,
        address refundAddress,
        string calldata destinationChain,
        string calldata destinationAddress
    ) external view returns (address);

    function addressForNativeUnwrap(
        bytes32 salt,
        address refundAddress,
        address recipient
    ) external view returns (address);

    function sendTokenDeposit(
        bytes32 salt,
        address refundAddress,
        string calldata destinationChain,
        string calldata destinationAddress,
        string calldata tokenSymbol
    ) external;

    function refundTokenDeposit(
        bytes32 salt,
        address refundAddress,
        string calldata destinationChain,
        string calldata destinationAddress,
        string calldata tokenSymbol,
        address[] calldata refundTokens
    ) external;

    function sendNativeDeposit(
        bytes32 salt,
        address refundAddress,
        string calldata destinationChain,
        string calldata destinationAddress
    ) external;

    function refundNativeDeposit(
        bytes32 salt,
        address refundAddress,
        string calldata destinationChain,
        string calldata destinationAddress,
        address[] calldata refundTokens
    ) external;

    function nativeUnwrap(
        bytes32 salt,
        address refundAddress,
        address payable recipient
    ) external;

    function refundNativeUnwrap(
        bytes32 salt,
        address refundAddress,
        address payable recipient,
        address[] calldata refundTokens
    ) external;

    function refundLockedAsset(
        address receiver,
        address token,
        uint256 amount
    ) external;

    function receiverImplementation() external returns (address receiver);

    function refundToken() external returns (address);
}
