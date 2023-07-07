// SPDX-License-Identifier: MIT

pragma solidity ^0.8.9;

import { IAxelarAuth } from './IAxelarAuth.sol';

interface IAxelarAuthWeighted is IAxelarAuth {
    error InvalidOperators();
    error InvalidThreshold();
    error DuplicateOperators();
    error MalformedSigners();
    error LowSignaturesWeight();
    error InvalidWeights();

    event OperatorshipTransferred(address[] newOperators, uint256[] newWeights, uint256 newThreshold);

    function currentEpoch() external view returns (uint256);

    function hashForEpoch(uint256 epoch) external view returns (bytes32);

    function epochForHash(bytes32 hash) external view returns (uint256);
}
