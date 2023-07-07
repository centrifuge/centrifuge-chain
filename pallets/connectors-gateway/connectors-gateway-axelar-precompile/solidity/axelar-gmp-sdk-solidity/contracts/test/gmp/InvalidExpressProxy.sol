// SPDX-License-Identifier: MIT

pragma solidity 0.8.9;

import { IExpressRegistry } from '../../interfaces/IExpressRegistry.sol';

contract InvalidExpressProxy {
    function registry() public pure returns (IExpressRegistry) {
        // return arbitrary address
        return IExpressRegistry(address(1));
    }
}
