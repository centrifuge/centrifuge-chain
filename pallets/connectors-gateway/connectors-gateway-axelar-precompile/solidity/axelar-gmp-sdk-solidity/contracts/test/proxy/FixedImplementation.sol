// SPDX-License-Identifier: MIT

pragma solidity ^0.8.0;

contract FixedImplementation {
    uint256 public value;

    function set(uint256 _val) external {
        value = _val;
    }
}
