// SPDX-License-Identifier: MIT

pragma solidity ^0.8.0;

import { IMultisig } from '../interfaces/IMultisig.sol';
import { MultisigBase } from '../auth/MultisigBase.sol';
import { Caller } from '../util/Caller.sol';

/**
 * @title Multisig Contract
 * @notice An extension of MultisigBase that can call functions on any contract.
 */
contract Multisig is Caller, MultisigBase, IMultisig {
    /**
     * @notice Contract constructor
     * @dev Sets the initial list of signers and corresponding threshold.
     * @param accounts Address array of the signers
     * @param threshold Signature threshold required to validate a transaction
     */
    constructor(address[] memory accounts, uint256 threshold) MultisigBase(accounts, threshold) {}

    /**
     * @notice Executes an external contract call.
     * @dev Calls a target address with specified calldata and optionally sends value.
     * This function is protected by the onlySigners modifier.
     * @param target The address of the contract to call
     * @param callData The data encoding the function and arguments to call
     * @param nativeValue The amount of native currency (e.g., ETH) to send along with the call
     */
    function execute(
        address target,
        bytes calldata callData,
        uint256 nativeValue
    ) external payable onlySigners {
        _call(target, callData, nativeValue);
    }

    /**
     * @notice Making contact able to receive native value
     */
    receive() external payable {}
}
