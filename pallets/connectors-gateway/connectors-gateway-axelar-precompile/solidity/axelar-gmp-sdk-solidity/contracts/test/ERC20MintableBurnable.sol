// SPDX-License-Identifier: MIT

pragma solidity ^0.8.0;

import { IERC20MintableBurnable } from '../interfaces/IERC20MintableBurnable.sol';
import { ERC20 } from './ERC20.sol';

contract ERC20MintableBurnable is ERC20, IERC20MintableBurnable {
    constructor(
        string memory name_,
        string memory symbol_,
        uint8 decimals_
    ) ERC20(name_, symbol_, decimals_) {}

    function burn(address account, uint256 amount) external {
        _approve(account, msg.sender, allowance[account][msg.sender] - amount);
        _burn(account, amount);
    }

    function mint(address account, uint256 amount) external {
        _mint(account, amount);
    }
}
