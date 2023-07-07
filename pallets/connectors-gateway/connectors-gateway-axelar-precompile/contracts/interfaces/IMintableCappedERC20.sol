// SPDX-License-Identifier: MIT

pragma solidity ^0.8.9;

import { IERC20 } from './IERC20.sol';
import { IERC20Permit } from './IERC20Permit.sol';
import { IOwnable } from './IOwnable.sol';

interface IMintableCappedERC20 is IERC20, IERC20Permit, IOwnable {
    error CapExceeded();

    function cap() external view returns (uint256);

    function mint(address account, uint256 amount) external;
}
