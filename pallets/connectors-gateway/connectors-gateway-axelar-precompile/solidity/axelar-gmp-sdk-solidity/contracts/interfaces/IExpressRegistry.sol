// SPDX-License-Identifier: MIT

pragma solidity ^0.8.0;

import { IAxelarGateway } from './IAxelarGateway.sol';

interface IExpressRegistry {
    error InvalidGateway();
    error NotExpressProxy();
    error AlreadyExpressCalled();

    function gateway() external returns (IAxelarGateway);

    function proxyCodeHash() external returns (bytes32);

    function registerExpressCallWithToken(
        address caller,
        string calldata sourceChain,
        string calldata sourceAddress,
        bytes32 payloadHash,
        string calldata tokenSymbol,
        uint256 amount
    ) external;

    function processExecuteWithToken(
        bytes32 commandId,
        string calldata sourceChain,
        string calldata sourceAddress,
        bytes calldata payload,
        string calldata tokenSymbol,
        uint256 amount
    ) external;
}
