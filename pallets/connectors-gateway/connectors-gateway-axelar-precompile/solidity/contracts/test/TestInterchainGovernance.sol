// SPDX-License-Identifier: MIT

pragma solidity ^0.8.0;

import { InterchainGovernance } from '../governance/InterchainGovernance.sol';

contract TestInterchainGovernance is InterchainGovernance {
    constructor(
        address gatewayAddress,
        string memory governanceChain_,
        string memory governanceAddress_,
        uint256 minimumTimeDelay
    ) InterchainGovernance(gatewayAddress, governanceChain_, governanceAddress_, minimumTimeDelay) {}

    function executeProposalAction(
        string calldata sourceChain,
        string calldata sourceAddress,
        bytes calldata payload
    ) external {
        _execute(sourceChain, sourceAddress, payload);
    }
}
