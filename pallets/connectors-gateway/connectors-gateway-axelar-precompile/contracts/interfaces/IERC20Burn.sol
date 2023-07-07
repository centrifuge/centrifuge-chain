// SPDX-License-Identifier: MIT

pragma solidity ^0.8.9;

interface IERC20Burn {
    function burn(bytes32 salt) external;
}
