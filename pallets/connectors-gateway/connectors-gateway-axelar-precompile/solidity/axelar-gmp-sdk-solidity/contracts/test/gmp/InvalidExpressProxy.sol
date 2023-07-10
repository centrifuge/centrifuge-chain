// SPDX-License-Identifier: MIT

<<<<<<< Updated upstream
pragma solidity 0.8.9;
=======
pragma solidity ^0.8.18;
>>>>>>> Stashed changes

import { IExpressRegistry } from '../../interfaces/IExpressRegistry.sol';

contract InvalidExpressProxy {
    function registry() public pure returns (IExpressRegistry) {
        // return arbitrary address
        return IExpressRegistry(address(1));
    }
}
