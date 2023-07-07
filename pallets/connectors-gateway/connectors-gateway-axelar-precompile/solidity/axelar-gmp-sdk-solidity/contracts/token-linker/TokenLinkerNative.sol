// SPDX-License-Identifier: MIT

pragma solidity ^0.8.0;

import { SafeNativeTransfer } from '../utils/SafeTransfer.sol';
import { TokenLinkerBase } from './TokenLinkerBase.sol';

contract TokenLinkerNative is TokenLinkerBase {
    using SafeNativeTransfer for address;

    error InsufficientBalance();
    error TranferFromNativeFailed();

    constructor(address gatewayAddress_, address gasServiceAddress_)
        TokenLinkerBase(gatewayAddress_, gasServiceAddress_)
    {}

    //keccak256('native_balance')
    uint256 public constant NATIVE_BALANCE_SLOT = 0x2b1b2f0e2e6377507cc7f28638bed85633f644ec5614112adcc88f3c5e87903a;

    function getNativeBalance() public view returns (uint256 nativeBalance) {
        assembly {
            nativeBalance := sload(NATIVE_BALANCE_SLOT)
        }
    }

    function _setNativeBalance(uint256 nativeBalance) internal {
        assembly {
            sstore(NATIVE_BALANCE_SLOT, nativeBalance)
        }
    }

    function _lockNative() internal pure override returns (bool) {
        return true;
    }

    function _giveToken(address to, uint256 amount) internal override {
        uint256 balance = getNativeBalance();
        if (balance < amount) revert InsufficientBalance();
        to.safeNativeTransfer(amount);
        _setNativeBalance(balance - amount);
    }

    function _takeToken(
        address, /*from*/
        uint256 amount
    ) internal override {
        uint256 balance = getNativeBalance();
        if (balance + amount > address(this).balance) revert TranferFromNativeFailed();
        _setNativeBalance(balance + amount);
    }

    function updateBalance() external payable {
        _setNativeBalance(address(this).balance);
    }
}
