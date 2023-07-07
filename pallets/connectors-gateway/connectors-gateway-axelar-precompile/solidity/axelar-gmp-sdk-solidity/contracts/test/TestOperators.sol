// SPDX-License-Identifier: MIT

pragma solidity ^0.8.0;

contract TestOperators {
    uint256 public num;

    event NumAdded(uint256 num);

    function setNum(uint256 _num) external payable returns (bool) {
        num = _num;

        emit NumAdded(_num);

        return true;
    }
}
