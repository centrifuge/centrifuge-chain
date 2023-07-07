// SPDX-License-Identifier: MIT

pragma solidity ^0.8.0;

// Simple contract that self-destructs in constructor
contract MockDepositReceiver {
    constructor(address refundAddress) {
        if (refundAddress == address(0)) refundAddress = msg.sender;

        selfdestruct(payable(refundAddress));
    }

    // @dev This function is for receiving Ether from unwrapping WETH9
    receive() external payable {}
}
