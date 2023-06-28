// SPDX-License-Identifier: MIT

pragma solidity ^0.8.9;

import { IAxelarGateway } from './IAxelarGateway.sol';
import { IERC20 } from './IERC20.sol';

abstract contract IAxelarForecallable {
    error NotApprovedByGateway();
    error AlreadyForecalled();
    error TransferFailed();

    IAxelarGateway public gateway;
    mapping(bytes32 => address) forecallers;

    constructor(address gatewayAddress) {
        gateway = IAxelarGateway(gatewayAddress);
    }

    function forecall(
        string calldata sourceChain,
        string calldata sourceAddress,
        bytes calldata payload,
        address forecaller
    ) external {
        _checkForecall(sourceChain, sourceAddress, payload, forecaller);
        if (forecallers[keccak256(abi.encode(sourceChain, sourceAddress, payload))] != address(0)) revert AlreadyForecalled();
        forecallers[keccak256(abi.encode(sourceChain, sourceAddress, payload))] = forecaller;
        _execute(sourceChain, sourceAddress, payload);
    }

    function execute(
        bytes32 commandId,
        string calldata sourceChain,
        string calldata sourceAddress,
        bytes calldata payload
    ) external {
        bytes32 payloadHash = keccak256(payload);
        if (!gateway.validateContractCall(commandId, sourceChain, sourceAddress, payloadHash)) revert NotApprovedByGateway();
        address forecaller = forecallers[keccak256(abi.encode(sourceChain, sourceAddress, payload))];
        if (forecaller != address(0)) {
            forecallers[keccak256(abi.encode(sourceChain, sourceAddress, payload))] = address(0);
        } else {
            _execute(sourceChain, sourceAddress, payload);
        }
    }

    function forecallWithToken(
        string calldata sourceChain,
        string calldata sourceAddress,
        bytes calldata payload,
        string calldata tokenSymbol,
        uint256 amount,
        address forecaller
    ) external {
        address token = gateway.tokenAddresses(tokenSymbol);
        uint256 amountPost = amountPostFee(amount, payload);
        _safeTransferFrom(token, msg.sender, amountPost);
        _checkForecallWithToken(sourceChain, sourceAddress, payload, tokenSymbol, amount, forecaller);
        if (forecallers[keccak256(abi.encode(sourceChain, sourceAddress, payload, tokenSymbol, amount))] != address(0))
            revert AlreadyForecalled();
        forecallers[keccak256(abi.encode(sourceChain, sourceAddress, payload, tokenSymbol, amount))] = forecaller;
        _executeWithToken(sourceChain, sourceAddress, payload, tokenSymbol, amountPost);
    }

    function executeWithToken(
        bytes32 commandId,
        string calldata sourceChain,
        string calldata sourceAddress,
        bytes calldata payload,
        string calldata tokenSymbol,
        uint256 amount
    ) external {
        bytes32 payloadHash = keccak256(payload);
        if (!gateway.validateContractCallAndMint(commandId, sourceChain, sourceAddress, payloadHash, tokenSymbol, amount))
            revert NotApprovedByGateway();
        address forecaller = forecallers[keccak256(abi.encode(sourceChain, sourceAddress, payload, tokenSymbol, amount))];
        if (forecaller != address(0)) {
            forecallers[keccak256(abi.encode(sourceChain, sourceAddress, payload, tokenSymbol, amount))] = address(0);
            address token = gateway.tokenAddresses(tokenSymbol);
            _safeTransfer(token, forecaller, amount);
        } else {
            _executeWithToken(sourceChain, sourceAddress, payload, tokenSymbol, amount);
        }
    }

    function _execute(
        string memory sourceChain,
        string memory sourceAddress,
        bytes calldata payload
    ) internal virtual {}

    function _executeWithToken(
        string memory sourceChain,
        string memory sourceAddress,
        bytes calldata payload,
        string memory tokenSymbol,
        uint256 amount
    ) internal virtual {}

    // Override this to keep a fee.
    function amountPostFee(
        uint256 amount,
        bytes calldata /*payload*/
    ) public virtual returns (uint256) {
        return amount;
    }

    // Override this and revert if you want to only allow certain people/calls to be able to forecall.
    function _checkForecall(
        string calldata sourceChain,
        string calldata sourceAddress,
        bytes calldata payload,
        address forecaller
    ) internal virtual {}

    // Override this and revert if you want to only allow certain people/calls to be able to forecall.
    function _checkForecallWithToken(
        string calldata sourceChain,
        string calldata sourceAddress,
        bytes calldata payload,
        string calldata tokenSymbol,
        uint256 amount,
        address forecaller
    ) internal virtual {}

    function _safeTransfer(
        address tokenAddress,
        address receiver,
        uint256 amount
    ) internal {
        (bool success, bytes memory returnData) = tokenAddress.call(abi.encodeWithSelector(IERC20.transfer.selector, receiver, amount));
        bool transferred = success && (returnData.length == uint256(0) || abi.decode(returnData, (bool)));

        if (!transferred || tokenAddress.code.length == 0) revert TransferFailed();
    }

    function _safeTransferFrom(
        address tokenAddress,
        address from,
        uint256 amount
    ) internal {
        (bool success, bytes memory returnData) = tokenAddress.call(
            abi.encodeWithSelector(IERC20.transferFrom.selector, from, address(this), amount)
        );
        bool transferred = success && (returnData.length == uint256(0) || abi.decode(returnData, (bool)));

        if (!transferred || tokenAddress.code.length == 0) revert TransferFailed();
    }
}