// SPDX-License-Identifier: MIT

pragma solidity ^0.8.0;

import { IProxy } from './IProxy.sol';

// General interface for upgradable contracts
interface IInitProxy is IProxy {
    function init(
        address implementationAddress,
        address newOwner,
        bytes memory params
    ) external;
}
