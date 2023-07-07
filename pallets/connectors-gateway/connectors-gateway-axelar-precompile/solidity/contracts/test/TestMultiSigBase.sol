// SPDX-License-Identifier: MIT

pragma solidity ^0.8.0;

import { MultisigBase } from '../auth/MultisigBase.sol';

contract TestMultiSigBase is MultisigBase {
    constructor(address[] memory accounts, uint256 threshold) MultisigBase(accounts, threshold) {}
}
