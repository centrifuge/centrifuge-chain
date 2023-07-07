// SPDX-License-Identifier: MIT

pragma solidity ^0.8.0;

import { IProxy } from '../interfaces/IProxy.sol';
import { IUpgradable } from '../interfaces/IUpgradable.sol';
import { BaseProxy } from './BaseProxy.sol';

/**
 * @title Proxy Contract
 * @notice A proxy contract that delegates calls to a designated implementation contract. Inherits from BaseProxy.
 * @dev The constructor takes in the address of the implementation contract, the owner address, and any optional setup
 * parameters for the implementation contract.
 */
contract Proxy is BaseProxy {
    /**
     * @notice Constructs the proxy contract with a the implementation address, owner address, and optional setup parameters.
     * @param implementationAddress The address of the implementation contract
     * @param owner The owner address
     * @param setupParams Optional parameters to setup the implementation contract
     * @dev The constructor verifies that the owner address is not the zero address and that the contract ID of the implementation is valid.
     * It then stores the implementation address and owner address in their designated storage slots and calls the setup function on the
     * implementation (if setup params exist).
     */
    constructor(
        address implementationAddress,
        address owner,
        bytes memory setupParams
    ) {
        if (owner == address(0)) revert InvalidOwner();

        bytes32 id = contractId();
        if (id != bytes32(0) && IUpgradable(implementationAddress).contractId() != id) revert InvalidImplementation();

        assembly {
            sstore(_IMPLEMENTATION_SLOT, implementationAddress)
            sstore(_OWNER_SLOT, owner)
        }

        if (setupParams.length != 0) {
            (bool success, ) = implementationAddress.delegatecall(
                abi.encodeWithSelector(IUpgradable.setup.selector, setupParams)
            );
            if (!success) revert SetupFailed();
        }
    }
}
