// SPDX-License-Identifier: MIT

pragma solidity ^0.8.0;

import { Upgradable } from '../upgradable/Upgradable.sol';

contract UpgradableTest is Upgradable {
    function contractId() external pure override returns (bytes32) {
        return keccak256('test');
    }
}
