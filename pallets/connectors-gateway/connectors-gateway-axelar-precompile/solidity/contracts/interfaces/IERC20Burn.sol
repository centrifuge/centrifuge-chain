// SPDX-License-Identifier: MIT

pragma solidity ^0.8.18;

interface IERC20Burn {
    function burn(bytes32 salt) external;
}
