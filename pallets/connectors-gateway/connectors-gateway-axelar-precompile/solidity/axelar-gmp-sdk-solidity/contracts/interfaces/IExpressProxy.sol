// SPDX-License-Identifier: MIT

pragma solidity ^0.8.0;

import { IAxelarExecutable } from './IAxelarExecutable.sol';
import { IExpressRegistry } from './IExpressRegistry.sol';

interface IExpressProxy is IAxelarExecutable {
    error NotExpressRegistry();
    error InvalidTokenSymbol();
    error ExpressCallNotAccepted();

    function registry() external view returns (IExpressRegistry);

    function deployRegistry(bytes calldata registryCreationCode) external;

    function expressExecuteWithToken(
        string calldata sourceChain,
        string calldata sourceAddress,
        bytes calldata payload,
        string calldata tokenSymbol,
        uint256 amount
    ) external;

    function completeExecuteWithToken(
        address expressCaller,
        bytes32 commandId,
        string calldata sourceChain,
        string calldata sourceAddress,
        bytes calldata payload,
        string calldata tokenSymbol,
        uint256 amount
    ) external;
}
