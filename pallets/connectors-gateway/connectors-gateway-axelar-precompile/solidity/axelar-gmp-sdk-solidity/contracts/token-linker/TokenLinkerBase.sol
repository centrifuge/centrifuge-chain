// SPDX-License-Identifier: MIT

pragma solidity ^0.8.0;

import { IAxelarGateway } from '../interfaces/IAxelarGateway.sol';
import { IAxelarGasService } from '../interfaces/IAxelarGasService.sol';
import { AxelarExecutable } from '../executable/AxelarExecutable.sol';
import { StringToAddress, AddressToString } from '../utils/AddressString.sol';
import { Upgradable } from '../upgradable/Upgradable.sol';

abstract contract TokenLinkerBase is AxelarExecutable, Upgradable {
    using StringToAddress for string;
    using AddressToString for address;

    bytes32 internal constant CONTRACT_ID = keccak256('token-linker');
    IAxelarGasService public immutable gasService;

    constructor(address gatewayAddress_, address gasServiceAddress_) AxelarExecutable(gatewayAddress_) {
        if (gasServiceAddress_ == address(0)) revert InvalidAddress();

        gasService = IAxelarGasService(gasServiceAddress_);
    }

    function contractId() external pure override returns (bytes32) {
        return CONTRACT_ID;
    }

    function sendToken(
        string calldata destinationChain,
        address to,
        uint256 amount,
        address refundAddress
    ) external payable virtual {
        _takeToken(msg.sender, amount);
        string memory thisAddress = address(this).toString();
        bytes memory payload = abi.encode(to, amount);
        uint256 gasValue = _lockNative() ? msg.value - amount : msg.value;
        if (gasValue > 0) {
            gasService.payNativeGasForContractCall{ value: gasValue }(
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
        (address recipient, uint256 amount) = abi.decode(payload, (address, uint256));
        _giveToken(recipient, amount);
    }

    function _lockNative() internal pure virtual returns (bool) {
        return false;
    }

    function _giveToken(address to, uint256 amount) internal virtual;

    function _takeToken(address from, uint256 amount) internal virtual;
}
