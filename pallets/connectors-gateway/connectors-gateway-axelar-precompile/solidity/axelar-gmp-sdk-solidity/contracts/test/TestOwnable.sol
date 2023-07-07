// SPDX-License-Identifier: MIT

pragma solidity ^0.8.0;

import { Ownable } from '../utils/Ownable.sol';

contract TestOwnable is Ownable {
    uint256 public num;

    event NumAdded(uint256 num);

    constructor() {
        address msgSender = msg.sender;
        assembly {
            sstore(_OWNER_SLOT, msgSender)
        }
        emit OwnershipTransferred(msgSender);
    }

    function setNum(uint256 _num) external payable onlyOwner returns (bool) {
        num = _num;

        emit NumAdded(_num);

        return true;
    }
}
