// SPDX-License-Identifier: MIT

pragma solidity ^0.8.0;

import { AxelarServiceGovernance } from '../governance/AxelarServiceGovernance.sol';

contract TestServiceGovernance is AxelarServiceGovernance {
    constructor(
        address gateway,
        string memory governanceChain_,
        string memory governanceAddress_,
        uint256 minimumTimeDelay,
        address[] memory signers,
        uint256 threshold
    ) AxelarServiceGovernance(gateway, governanceChain_, governanceAddress_, minimumTimeDelay, signers, threshold) {}

    function executeProposalAction(
        string calldata sourceChain,
        string calldata sourceAddress,
        bytes calldata payload
    ) external {
        _execute(sourceChain, sourceAddress, payload);
    }
}
