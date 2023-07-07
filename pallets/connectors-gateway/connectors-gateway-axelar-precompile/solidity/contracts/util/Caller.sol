// SPDX-License-Identifier: MIT

pragma solidity ^0.8.0;

import { ICaller } from '../interfaces/ICaller.sol';

contract Caller is ICaller {
    /**
     * @dev Calls a target address with specified calldata and optionally sends value.
     */
    function _call(
        address target,
        bytes calldata callData,
        uint256 nativeValue
    ) internal {
        if (nativeValue > address(this).balance) revert InsufficientBalance();

        (bool success, ) = target.call{ value: nativeValue }(callData);
        if (!success) {
            revert ExecutionFailed();
        }
    }
}
