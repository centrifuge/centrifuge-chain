// SPDX-License-Identifier: MIT

<<<<<<< Updated upstream
pragma solidity 0.8.9;
=======
pragma solidity ^0.8.18;
>>>>>>> Stashed changes

import { Proxy } from '../../upgradable/Proxy.sol';
import { IUpgradable } from '../../interfaces/IUpgradable.sol';

contract TestProxy is Proxy {
    constructor(
        address implementationAddress,
        address owner,
        bytes memory setupParams
    ) Proxy(implementationAddress, owner, setupParams) {}

    function contractId() internal pure override returns (bytes32) {
        return keccak256('proxy-implementation');
    }
}
