// SPDX-License-Identifier: MIT

pragma solidity ^0.8.0;

import { IProxy } from '../interfaces/IProxy.sol';

/**
 * @title FixedProxy Contract
 * @notice The FixedProxy is a type of Proxy contract with a fixed implementation that cannot be updated.
 * It implements the IProxy interface. Any function calls to this contract will be forwarded to the implementation contract.
 */
contract FixedProxy is IProxy {
    /**
     * @dev The immutable address of the implementation contract.
     * This address is set in the constructor and cannot be updated after.
     */
    address public immutable implementation;

    /**
     * @dev Constructs a FixedProxy contract with the given implementation address.
     * @param implementationAddress The address of the implementation contract
     */
    constructor(address implementationAddress) {
        implementation = implementationAddress;
    }

    /**
     * @dev Shadows the setup function of the implementation contract so it can't be called directly via the proxy.
     * @param setupParams The setup parameters for the implementation contract.
     */
    function setup(bytes calldata setupParams) external {}

    /**
     * @dev Fallback function that delegates all calls to the implementation contract.
     * If the call fails, it reverts with the returned error data. If it succeeds, it returns the data from the call.
     */
    fallback() external payable virtual {
        address implementation_ = implementation;

        assembly {
            calldatacopy(0, 0, calldatasize())

            let result := delegatecall(gas(), implementation_, 0, calldatasize(), 0, 0)
            returndatacopy(0, 0, returndatasize())

            switch result
            case 0 {
                revert(0, returndatasize())
            }
            default {
                return(0, returndatasize())
            }
        }
    }

    /**
     * @dev Payable fallback function. Can be overridden in derived contracts.
     */
    receive() external payable virtual {}
}
