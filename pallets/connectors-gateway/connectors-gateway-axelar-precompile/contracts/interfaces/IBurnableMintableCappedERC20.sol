// SPDX-License-Identifier: MIT

pragma solidity ^0.8.9;

import { IERC20Burn } from './IERC20Burn.sol';
import { IERC20BurnFrom } from './IERC20BurnFrom.sol';
import { IMintableCappedERC20 } from './IMintableCappedERC20.sol';

interface IBurnableMintableCappedERC20 is IERC20Burn, IERC20BurnFrom, IMintableCappedERC20 {
    function depositAddress(bytes32 salt) external view returns (address);

    function burn(bytes32 salt) external;

    function burnFrom(address account, uint256 amount) external;
}
