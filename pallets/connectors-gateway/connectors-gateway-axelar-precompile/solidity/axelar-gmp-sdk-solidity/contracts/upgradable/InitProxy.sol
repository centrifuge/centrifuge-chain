// SPDX-License-Identifier: MIT

pragma solidity ^0.8.0;

import { IInitProxy } from '../interfaces/IInitProxy.sol';
import { IUpgradable } from '../interfaces/IUpgradable.sol';
import { BaseProxy } from './BaseProxy.sol';

/**
 * @title InitProxy Contract
 * @notice A proxy contract that can be initialized to use a specified implementation and owner. Inherits from BaseProxy
 * and implements the IInitProxy interface.
 * @dev This proxy is constructed empty and then later initialized with the implementation contract address, new owner address,
 * and any optional setup parameters.
 */
contract InitProxy is BaseProxy, IInitProxy {
    /**
     * @dev Initializes the contract and sets the caller as the owner of the contract.
     */
    constructor() {
        assembly {
            sstore(_OWNER_SLOT, caller())
        }
    }

    /**
     * @notice Initializes the proxy contract with the specified implementation, new owner, and any optional setup parameters.
     * @param implementationAddress The address of the implementation contract
     * @param newOwner The address of the new proxy owner
     * @param params Optional parameters to be passed to the setup function of the implementation contract
     * @dev This function is only callable by the owner of the proxy. If the proxy has already been initialized, it will revert.
     * If the contract ID of the implementation is incorrect, it will also revert. It then stores the implementation address and
     * new owner address in the designated storage slots and calls the setup function on the implementation (if setup params exist).
     */
    function init(
        address implementationAddress,
        address newOwner,
        bytes memory params
    ) external {
        address owner;

        assembly {
            owner := sload(_OWNER_SLOT)
        }

        if (msg.sender != owner) revert NotOwner();
        if (implementation() != address(0)) revert AlreadyInitialized();

        bytes32 id = contractId();
        if (id != bytes32(0) && IUpgradable(implementationAddress).contractId() != id) revert InvalidImplementation();

        assembly {
            sstore(_IMPLEMENTATION_SLOT, implementationAddress)
            sstore(_OWNER_SLOT, newOwner)
        }

        if (params.length != 0) {
            (bool success, ) = implementationAddress.delegatecall(
                abi.encodeWithSelector(IUpgradable.setup.selector, params)
            );
            if (!success) revert SetupFailed();
        }
    }
}
