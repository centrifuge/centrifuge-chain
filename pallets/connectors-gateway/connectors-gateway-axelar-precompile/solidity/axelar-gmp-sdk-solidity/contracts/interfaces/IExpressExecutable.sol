// SPDX-License-Identifier: MIT

pragma solidity ^0.8.0;

import { IAxelarExecutable } from './IAxelarExecutable.sol';

interface IExpressExecutable is IAxelarExecutable {
    function acceptExpressCallWithToken(
        address caller,
        string calldata sourceChain,
        string calldata sourceAddress,
        bytes32 payloadHash,
        string calldata tokenSymbol,
        uint256 amount
    ) external view returns (bool);
}
