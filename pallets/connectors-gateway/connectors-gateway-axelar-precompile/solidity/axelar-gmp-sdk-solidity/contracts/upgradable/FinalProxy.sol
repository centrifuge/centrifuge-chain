// SPDX-License-Identifier: MIT

pragma solidity ^0.8.0;

import { IProxy } from '../interfaces/IProxy.sol';
import { IFinalProxy } from '../interfaces/IFinalProxy.sol';
import { Create3 } from '../deploy/Create3.sol';
import { BaseProxy } from './BaseProxy.sol';
import { Proxy } from './Proxy.sol';

/**
 * @title FinalProxy Contract
 * @notice The FinalProxy contract is a proxy that can be upgraded to a final implementation
 * that uses less gas than regular proxy calls. It inherits from the Proxy contract and implements
 * the IFinalProxy interface.
 */
contract FinalProxy is Proxy, IFinalProxy {
    bytes32 internal constant FINAL_IMPLEMENTATION_SALT = keccak256('final-implementation');

    /**
     * @dev Constructs a FinalProxy contract with a given implementation address, owner, and setup parameters.
     * @param implementationAddress The address of the implementation contract
     * @param owner The owner of the proxy contract
     * @param setupParams Parameters to setup the implementation contract
     */
    constructor(
        address implementationAddress,
        address owner,
        bytes memory setupParams
    ) Proxy(implementationAddress, owner, setupParams) {}

    /**
     * @dev The final implementation address takes less gas to compute than reading an address from storage. That makes FinalProxy
     * more efficient when making delegatecalls to the implementation (assuming it is the final implementation).
     * @return implementation_ The address of the final implementation if it exists, otherwise the current implementation
     */
    function implementation() public view override(BaseProxy, IProxy) returns (address implementation_) {
        implementation_ = _finalImplementation();
        if (implementation_ == address(0)) {
            implementation_ = super.implementation();
        }
    }

    /**
     * @dev Checks if the final implementation has been deployed.
     * @return bool True if the final implementation exists, false otherwise
     */
    function isFinal() public view returns (bool) {
        return _finalImplementation() != address(0);
    }

    /**
     * @dev Computes the final implementation address.
     * @return implementation_ The address of the final implementation, or the zero address if the final implementation
     * has not yet been deployed
     */
    function _finalImplementation() internal view virtual returns (address implementation_) {
        /**
         * @dev Computing the address is cheaper than using storage
         */
        implementation_ = Create3.deployedAddress(address(this), FINAL_IMPLEMENTATION_SALT);

        if (implementation_.code.length == 0) implementation_ = address(0);
    }

    /**
     * @dev Upgrades the proxy to a final implementation.
     * @param bytecode The bytecode of the final implementation contract
     * @param setupParams The parameters to setup the final implementation contract
     * @return finalImplementation_ The address of the final implementation contract
     */
    function finalUpgrade(bytes memory bytecode, bytes calldata setupParams)
        public
        returns (address finalImplementation_)
    {
        address owner;
        assembly {
            owner := sload(_OWNER_SLOT)
        }
        if (msg.sender != owner) revert NotOwner();

        finalImplementation_ = Create3.deploy(FINAL_IMPLEMENTATION_SALT, bytecode);
        if (setupParams.length != 0) {
            (bool success, ) = finalImplementation_.delegatecall(
                abi.encodeWithSelector(BaseProxy.setup.selector, setupParams)
            );
            if (!success) revert SetupFailed();
        }
    }
}
