// SPDX-License-Identifier: MIT

pragma solidity ^0.8.0;

import { TimeLock } from '../utils/TimeLock.sol';

contract TimeLockTest is TimeLock {
    uint256 public num;

    event NumUpdated(uint256 newNum);

    constructor(uint256 _minimumTimeDelay) TimeLock(_minimumTimeDelay) {}

    function getNum() external view returns (uint256) {
        return num;
    }

    function scheduleSetNum(uint256 _num, uint256 _eta) external {
        bytes32 hash = keccak256(abi.encodePacked(_num));

        _scheduleTimeLock(hash, _eta);
    }

    function scheduleSetNumByHash(bytes32 _hash, uint256 _eta) external {
        _scheduleTimeLock(_hash, _eta);
    }

    function cancelSetNum(uint256 _num) external {
        bytes32 hash = keccak256(abi.encodePacked(_num));

        _cancelTimeLock(hash);
    }

    function cancelSetNumByHash(bytes32 _hash) external {
        _cancelTimeLock(_hash);
    }

    function setNum(uint256 _num) external {
        bytes32 hash = keccak256(abi.encodePacked(_num));

        _finalizeTimeLock(hash);

        num = _num;

        emit NumUpdated(_num);
    }

    function setNumByHash(bytes32 _hash, uint256 _num) external {
        _finalizeTimeLock(_hash);

        num = _num;

        emit NumUpdated(_num);
    }
}
