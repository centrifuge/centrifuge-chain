// SPDX-License-Identifier: MIT

pragma solidity 0.8.9;

import { ExpressExecutable } from '../../express/ExpressExecutable.sol';
import { IAxelarGateway } from '../../interfaces/IAxelarGateway.sol';

contract ExecutableSample is ExpressExecutable {
    string public value;
    string public sourceChain;
    string public sourceAddress;

    constructor(address gateway_) ExpressExecutable(gateway_) {}

    // Call this function to update the value of this contract along with all its siblings'.
    function setRemoteValue(
        string calldata destinationChain,
        string calldata destinationAddress,
        string calldata value_
    ) external payable {
        bytes memory payload = abi.encode(value_);

        gateway.callContract(destinationChain, destinationAddress, payload);
    }

    // Handles calls created by setAndSend. Updates this contract's value
    function _execute(
        string calldata sourceChain_,
        string calldata sourceAddress_,
        bytes calldata payload_
    ) internal override {
        (value) = abi.decode(payload_, (string));
        sourceChain = sourceChain_;
        sourceAddress = sourceAddress_;
    }
}
