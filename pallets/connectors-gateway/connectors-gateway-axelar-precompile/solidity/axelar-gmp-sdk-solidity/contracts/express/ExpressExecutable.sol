// SPDX-License-Identifier: MIT

pragma solidity ^0.8.0;

import { IAxelarGateway } from '../interfaces/IAxelarGateway.sol';
import { IERC20 } from '../interfaces/IERC20.sol';
import { IExpressExecutable } from '../interfaces/IExpressExecutable.sol';

/**
 * @title ExpressExecutable
 * @dev This abstract contract provides base functionalities for express executions. Express Executable
 * contracts should inhererit this abstract contract and implement custom logic for all virtual functions.
 */
abstract contract ExpressExecutable is IExpressExecutable {
    IAxelarGateway public immutable gateway;

    /**
     * @dev Sets the address for the AxelarGateway.
     * @param gateway_ The address of the AxelarGateway contract.
     */
    constructor(address gateway_) {
        if (gateway_ == address(0)) revert InvalidAddress();

        gateway = IAxelarGateway(gateway_);
    }

    /**
     * @dev Checks if the express call can be accepted.
     * param caller The address of the caller.
     * param sourceChain The source blockchain.
     * param sourceAddress The source address.
     * param payloadHash The payload hash.
     * param tokenSymbol The token symbol.
     * param amount The token amount.
     * @return bool boolean representing whether the express call can be accepted.
     * This is a virtual function and should be implemented in derived contracts.
     */
    function acceptExpressCallWithToken(
        address, /*caller*/
        string calldata, /*sourceChain*/
        string calldata, /*sourceAddress*/
        bytes32, /*payloadHash*/
        string calldata, /*tokenSymbol*/
        uint256 /*amount*/
    ) external view virtual returns (bool) {
        return true;
    }

    /**
     * @notice This function is shadowed by the proxy and can be called only internally.
     * @dev Executes the call.
     * @param sourceChain The source blockchain.
     * @param sourceAddress The source address.
     * @param payload The payload.
     */
    function execute(
        bytes32,
        string calldata sourceChain,
        string calldata sourceAddress,
        bytes calldata payload
    ) external {
        _execute(sourceChain, sourceAddress, payload);
    }

    /**
     * @notice This function is shadowed by the proxy and can be called only internally.
     * @dev Executes the call with token.
     * @param sourceChain The source blockchain.
     * @param sourceAddress The source address.
     * @param payload The payload.
     * @param tokenSymbol The token symbol.
     * @param amount The token amount.
     */
    function executeWithToken(
        bytes32,
        string calldata sourceChain,
        string calldata sourceAddress,
        bytes calldata payload,
        string calldata tokenSymbol,
        uint256 amount
    ) external {
        _executeWithToken(sourceChain, sourceAddress, payload, tokenSymbol, amount);
    }

    /**
     * @dev Defines the logic to execute the call.
     * @param sourceChain The source blockchain.
     * @param sourceAddress The source address.
     * @param payload The payload.
     * This is a virtual function and needs to be implemented in the derived contracts.
     */
    function _execute(
        string calldata sourceChain,
        string calldata sourceAddress,
        bytes calldata payload
    ) internal virtual {}

    /**
     * @dev Defines the steps to execute the call with token.
     * @param sourceChain The source blockchain.
     * @param sourceAddress The source address.
     * @param payload The payload.
     * @param tokenSymbol The token symbol.
     * @param amount The token amount.
     * This is a virtual function and needs to be implemented in the derived contracts.
     */
    function _executeWithToken(
        string calldata sourceChain,
        string calldata sourceAddress,
        bytes calldata payload,
        string calldata tokenSymbol,
        uint256 amount
    ) internal virtual {}
}
