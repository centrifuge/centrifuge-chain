// SPDX-License-Identifier: MIT

pragma solidity ^0.8.9;

import { IERC20 } from './IERC20.sol';

// WETH9 specific interface
interface IWETH9 is IERC20 {
    function deposit() external payable;

    function withdraw(uint256 amount) external;
}
