// SPDX-License-Identifier: MIT

pragma solidity 0.8.9;

import { Proxy } from '../util/Proxy.sol';

contract AxelarDepositServiceProxy is Proxy {
    function contractId() internal pure override returns (bytes32) {
        return keccak256('axelar-deposit-service');
    }

    // @dev This function is for receiving refunds when refundAddress was 0x0
    // solhint-disable-next-line no-empty-blocks
    receive() external payable override {}
}
