// SPDX-License-Identifier: MIT

pragma solidity ^0.8.0;

import { ERC20MintableBurnable } from './ERC20MintableBurnable.sol';

contract ERC20MintableBurnableInit is ERC20MintableBurnable {
    constructor(uint8 decimals) ERC20MintableBurnable('', '', decimals) {}

    function init(string memory name_, string memory symbol_) external {
        name = name_;
        symbol = symbol_;
    }
}
