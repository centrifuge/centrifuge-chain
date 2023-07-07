// SPDX-License-Identifier: MIT

pragma solidity ^0.8.0;

import { IUpgradable } from '../../interfaces/IUpgradable.sol';

contract DifferentProxyImplementation is IUpgradable {
    uint256 public value;
    string public name;

    function owner() external pure override returns (address) {
        return address(0);
    }

    function transferOwnership(address) external override {}

    function pendingOwner() external pure returns (address) {
        return address(0);
    }

    function proposeOwnership(address newOwner) external {}

    function acceptOwnership() external {}

    function implementation() external pure override returns (address) {
        return address(0);
    }

    function upgrade(
        address,
        bytes32,
        bytes calldata
    ) external override {}

    function set(uint256 _val) external {
        value = _val;
    }

    function setup(bytes calldata _data) external {
        (uint256 initVal, string memory _name) = abi.decode(_data, (uint256, string));
        value = initVal;
        name = _name;
    }

    function contractId() external pure returns (bytes32) {
        return keccak256('proxy-implementation');
    }

    function different() external pure returns (bool) {
        return true;
    }
}
