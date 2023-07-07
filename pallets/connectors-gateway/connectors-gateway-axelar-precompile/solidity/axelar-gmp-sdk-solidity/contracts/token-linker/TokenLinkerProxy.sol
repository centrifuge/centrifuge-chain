// SPDX-License-Identifier: MIT

pragma solidity ^0.8.0;

import { Proxy } from '../upgradable/Proxy.sol';

contract TokenLinkerProxy is Proxy {
    bytes32 internal constant CONTRACT_ID = keccak256('token-linker');

    constructor(
        address implementationAddress,
        address owner,
        bytes memory setupParams
    ) Proxy(implementationAddress, owner, setupParams) {}

    function contractId() internal pure override returns (bytes32) {
        return CONTRACT_ID;
    }

    receive() external payable override {}
}
