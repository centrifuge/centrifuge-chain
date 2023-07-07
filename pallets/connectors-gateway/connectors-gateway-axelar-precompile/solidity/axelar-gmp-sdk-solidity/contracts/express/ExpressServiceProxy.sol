// SPDX-License-Identifier: MIT

pragma solidity ^0.8.18;

import { FinalProxy } from '../upgradable/FinalProxy.sol';

/**
 * @title ExpressServiceProxy
 * @notice This contract acts as a proxy for the ExpressService contract.
 * @dev It extends the FinalProxy contract, and as such inherits its upgradeability features.
 */
contract ExpressServiceProxy is FinalProxy {
    /**
     * @notice Constructor for creating a new ExpressServiceProxy contract.
     * @param implementationAddress The implementation address of the ExpressService contract.
     * @param owner The owner of the proxy contract.
     * @param setupParams Additional parameters that can be used for setting up the implementation.
     */
    constructor(
        address implementationAddress,
        address owner,
        bytes memory setupParams
    ) FinalProxy(implementationAddress, owner, setupParams) {}

    /**
     * @notice Returns the ID of the contract. This ID is used to distinguish this contract within a system.
     * @return bytes32 ID of the contract.
     */
    function contractId() internal pure override returns (bytes32) {
        return keccak256('axelar-gmp-express-service');
    }
}
