// SPDX-License-Identifier: MIT

pragma solidity 0.8.9;

import { SafeTokenTransfer } from '@axelar-network/axelar-gmp-sdk-solidity/contracts/utils/SafeTransfer.sol';
import { StringToBytes32, Bytes32ToString } from '@axelar-network/axelar-gmp-sdk-solidity/contracts/utils/Bytes32String.sol';
import { IAxelarGateway } from '../interfaces/IAxelarGateway.sol';
import { IDepositServiceBase } from '../interfaces/IDepositServiceBase.sol';

// This should be owned by the microservice that is paying for gas.
abstract contract DepositServiceBase is IDepositServiceBase {
    using SafeTokenTransfer for address;
    using StringToBytes32 for string;
    using Bytes32ToString for bytes32;

    // Using immutable storage to keep the constants in the bytecode
    address public immutable gateway;
    address public immutable wrappedTokenAddress;
    bytes32 internal immutable wrappedSymbolBytes;

    constructor(address gateway_, string memory wrappedSymbol_) {
        if (gateway_ == address(0)) revert InvalidAddress();

        bool wrappedTokenEnabled = bytes(wrappedSymbol_).length > 0;

        gateway = gateway_;
        wrappedTokenAddress = wrappedTokenEnabled ? IAxelarGateway(gateway_).tokenAddresses(wrappedSymbol_) : address(0);
        wrappedSymbolBytes = wrappedTokenEnabled ? wrappedSymbol_.toBytes32() : bytes32(0);

        // Wrapped token symbol param is optional
        // When specified we are checking if token exists in the gateway
        if (wrappedTokenEnabled && wrappedTokenAddress == address(0)) revert InvalidSymbol();
    }

    function wrappedToken() public view returns (address) {
        return wrappedTokenAddress;
    }

    // @dev Converts bytes32 from immutable storage into a string
    function wrappedSymbol() public view returns (string memory) {
        return wrappedSymbolBytes.toTrimmedString();
    }
}
