// SPDX-License-Identifier: MIT

pragma solidity ^0.8.0;

import { IERC20 } from '../interfaces/IERC20.sol';
import { SafeTokenTransfer, SafeTokenTransferFrom } from '../utils/SafeTransfer.sol';
import { TokenLinkerBase } from './TokenLinkerBase.sol';

contract TokenLinkerLockUnlock is TokenLinkerBase {
    using SafeTokenTransfer for IERC20;
    using SafeTokenTransferFrom for IERC20;

    address public immutable tokenAddress;

    constructor(
        address gatewayAddress_,
        address gasServiceAddress_,
        address tokenAddress_
    ) TokenLinkerBase(gatewayAddress_, gasServiceAddress_) {
        if (tokenAddress_ == address(0)) revert InvalidAddress();

        tokenAddress = tokenAddress_;
    }

    function _giveToken(address to, uint256 amount) internal override {
        IERC20(tokenAddress).safeTransfer(to, amount);
    }

    function _takeToken(address from, uint256 amount) internal override {
        IERC20(tokenAddress).safeTransferFrom(from, address(this), amount);
    }
}
