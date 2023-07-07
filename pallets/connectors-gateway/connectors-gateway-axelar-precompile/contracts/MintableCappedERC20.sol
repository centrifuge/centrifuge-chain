// SPDX-License-Identifier: MIT

pragma solidity 0.8.9;

import { IMintableCappedERC20 } from './interfaces/IMintableCappedERC20.sol';

import { ERC20 } from './ERC20.sol';
import { ERC20Permit } from './ERC20Permit.sol';
import { Ownable } from './Ownable.sol';

contract MintableCappedERC20 is IMintableCappedERC20, ERC20, ERC20Permit, Ownable {
    uint256 public immutable cap;

    constructor(
        string memory name,
        string memory symbol,
        uint8 decimals,
        uint256 capacity
    ) ERC20(name, symbol, decimals) ERC20Permit(name) Ownable() {
        cap = capacity;
    }

    function mint(address account, uint256 amount) external onlyOwner {
        uint256 capacity = cap;

        _mint(account, amount);

        if (capacity == 0) return;

        if (totalSupply > capacity) revert CapExceeded();
    }
}
