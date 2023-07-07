// SPDX-License-Identifier: MIT

pragma solidity ^0.8.0;

import { StringToAddress, AddressToString } from '../utils/AddressString.sol';
import { Bytes32ToString, StringToBytes32 } from '../utils/Bytes32String.sol';

contract UtilTest {
    using AddressToString for address;
    using StringToAddress for string;
    using Bytes32ToString for bytes32;
    using StringToBytes32 for string;

    function addressToString(address address_) external pure returns (string memory) {
        return address_.toString();
    }

    function stringToAddress(string calldata string_) external pure returns (address) {
        return string_.toAddress();
    }

    function bytes32ToString(bytes32 bytes_) external pure returns (string memory) {
        return bytes_.toTrimmedString();
    }

    function stringToBytes32(string calldata string_) external pure returns (bytes32) {
        return string_.toBytes32();
    }
}
