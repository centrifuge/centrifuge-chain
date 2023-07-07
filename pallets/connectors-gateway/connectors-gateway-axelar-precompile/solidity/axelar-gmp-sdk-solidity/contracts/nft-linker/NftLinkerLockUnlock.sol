// SPDX-License-Identifier: MIT

pragma solidity ^0.8.0;

import { IERC721 } from '../interfaces/IERC721.sol';
import { NftLinkerBase } from './NftLinkerBase.sol';

contract NftLinkerLockUnlock is NftLinkerBase {
    error TransferFailed();
    error TransferFromFailed();

    address public immutable operatorAddress;

    constructor(
        address gatewayAddress_,
        address gasServiceAddress_,
        address operatorAddress_
    ) NftLinkerBase(gatewayAddress_, gasServiceAddress_) {
        operatorAddress = operatorAddress_;
    }

    function _giveNft(address to, uint256 tokenId) internal override {
        (bool success, bytes memory returnData) = operatorAddress.call(
            abi.encodeWithSelector(IERC721.transferFrom.selector, address(this), to, tokenId)
        );
        bool transferred = success && (returnData.length == uint256(0) || abi.decode(returnData, (bool)));

        if (!transferred || operatorAddress.code.length == 0) revert TransferFailed();
    }

    function _takeNft(address from, uint256 tokenId) internal override {
        (bool success, bytes memory returnData) = operatorAddress.call(
            abi.encodeWithSelector(IERC721.transferFrom.selector, from, address(this), tokenId)
        );
        bool transferred = success && (returnData.length == uint256(0) || abi.decode(returnData, (bool)));

        if (!transferred || operatorAddress.code.length == 0) revert TransferFromFailed();
    }
}
