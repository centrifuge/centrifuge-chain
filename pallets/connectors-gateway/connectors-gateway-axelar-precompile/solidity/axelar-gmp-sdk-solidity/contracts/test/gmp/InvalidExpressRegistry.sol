// SPDX-License-Identifier: MIT

pragma solidity ^0.8.0;

import { IAxelarGateway } from '../../interfaces/IAxelarGateway.sol';

contract InvalidExpressRegistry {
    IAxelarGateway public immutable gateway;
    bytes32 public immutable proxyCodeHash;

    error InvalidGateway();

    mapping(bytes32 => address) private expressCallsWithToken;

    constructor(address gateway_, address proxy_) {
        if (gateway_ == address(0)) revert InvalidGateway();

        gateway = IAxelarGateway(gateway_);
        proxyCodeHash = proxy_.codehash;
    }
}
