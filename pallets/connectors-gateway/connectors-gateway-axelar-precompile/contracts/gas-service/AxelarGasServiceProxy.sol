// SPDX-License-Identifier: MIT

pragma solidity 0.8.9;

import { Proxy } from '../util/Proxy.sol';
import { IUpgradable } from '../interfaces/IUpgradable.sol';

contract AxelarGasServiceProxy is Proxy {
    function contractId() internal pure override returns (bytes32) {
        return keccak256('axelar-gas-service');
    }
}
