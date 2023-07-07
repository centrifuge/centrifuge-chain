// SPDX-License-Identifier: MIT

pragma solidity ^0.8.0;

import { IAxelarGateway } from '../interfaces/IAxelarGateway.sol';
import { IAxelarGasService } from '../interfaces/IAxelarGasService.sol';
import { AxelarExecutable } from '../executable/AxelarExecutable.sol';
import { AddressToString, StringToAddress } from '../utils/AddressString.sol';
import { Upgradable } from '../upgradable/Upgradable.sol';

abstract contract NftLinkerBase is AxelarExecutable, Upgradable {
    using StringToAddress for string;
    using AddressToString for address;

    bytes32 internal constant CONTRACT_ID = keccak256('nft-linker');
    IAxelarGasService public immutable gasService;

    constructor(address gatewayAddress, address gasServiceAddress_) AxelarExecutable(gatewayAddress) {
        gasService = IAxelarGasService(gasServiceAddress_);
    }

    function contractId() external pure override returns (bytes32) {
        return CONTRACT_ID;
    }

    function sendNft(
        string memory destinationChain,
        address to,
        uint256 tokenId,
        address refundAddress
    ) external payable virtual {
        string memory thisAddress = address(this).toString();
        _takeNft(msg.sender, tokenId);
        bytes memory payload = abi.encode(to, tokenId);
        if (msg.value > 0) {
            gasService.payNativeGasForContractCall{ value: msg.value }(
                address(this),
                destinationChain,
                thisAddress,
                payload,
                refundAddress
            );
        }
        gateway.callContract(destinationChain, thisAddress, payload);
    }

    function _execute(
        string calldata, /*sourceChain*/
        string calldata sourceAddress,
        bytes calldata payload
    ) internal override {
        if (sourceAddress.toAddress() != address(this)) return;
        (address recipient, uint256 tokenId) = abi.decode(payload, (address, uint256));
        _giveNft(recipient, tokenId);
    }

    function _giveNft(address to, uint256 tokenId) internal virtual;

    function _takeNft(address from, uint256 tokenId) internal virtual;
}
