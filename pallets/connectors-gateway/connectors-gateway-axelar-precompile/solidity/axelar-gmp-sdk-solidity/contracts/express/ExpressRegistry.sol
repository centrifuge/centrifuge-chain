// SPDX-License-Identifier: MIT

pragma solidity ^0.8.0;

import { IAxelarGateway } from '../interfaces/IAxelarGateway.sol';
import { IExpressProxy } from '../interfaces/IExpressProxy.sol';
import { IExpressRegistry } from '../interfaces/IExpressRegistry.sol';

/**
 * @title ExpressRegistry
 * @notice A registry contract for tracking express calls with tokens.
 * @dev It implements the IExpressRegistry interface and interacts with the AxelarGateway and ExpressProxy contracts.
 */
contract ExpressRegistry is IExpressRegistry {
    IAxelarGateway public immutable gateway;
    bytes32 public immutable proxyCodeHash;

    mapping(bytes32 => address) private expressCallsWithToken;

    /**
     * @notice Constructor for creating a new ExpressRegistry contract.
     * @param gateway_ The instance of the AxelarGateway contract.
     * @param proxy_ The instance of the ExpressProxy contract.
     */
    constructor(address gateway_, address proxy_) {
        if (gateway_ == address(0)) revert InvalidGateway();

        gateway = IAxelarGateway(gateway_);
        proxyCodeHash = proxy_.codehash;
    }

    /**
     * @notice Registers an express call with a token, fails if call has already been registered.
     * @param expressCaller The address of the express caller.
     * @param sourceChain The source chain of the call.
     * @param sourceAddress The source address of the call.
     * @param payloadHash The hash of the payload.
     * @param tokenSymbol The symbol of the token associated with the call.
     * @param amount The amount of the token associated with the call.
     */
    function registerExpressCallWithToken(
        address expressCaller,
        string calldata sourceChain,
        string calldata sourceAddress,
        bytes32 payloadHash,
        string calldata tokenSymbol,
        uint256 amount
    ) external {
        _onlyProxy();

        (bytes32 slot, address existingExpressCaller) = _getExpressCallWithToken(
            sourceChain,
            sourceAddress,
            msg.sender,
            payloadHash,
            tokenSymbol,
            amount
        );

        if (existingExpressCaller != address(0)) revert AlreadyExpressCalled();

        _setExpressCallWithToken(slot, expressCaller);
    }

    /**
     * @notice Processes an express call with token by calling completeExecuteWithToken on ExpressProxy.
     * @param commandId The ID of the command.
     * @param sourceChain The source chain of the call.
     * @param sourceAddress The source address of the call.
     * @param payload The payload of the call.
     * @param tokenSymbol The symbol of the token associated with the call.
     * @param amount The amount of the token associated with the call.
     */
    function processExecuteWithToken(
        bytes32 commandId,
        string calldata sourceChain,
        string calldata sourceAddress,
        bytes calldata payload,
        string calldata tokenSymbol,
        uint256 amount
    ) external {
        _onlyProxy();

        bytes32 payloadHash = keccak256(payload);
        (bytes32 slot, address expressCaller) = _getExpressCallWithToken(
            sourceChain,
            sourceAddress,
            msg.sender,
            payloadHash,
            tokenSymbol,
            amount
        );

        if (
            gateway.isContractCallAndMintApproved(
                commandId,
                sourceChain,
                sourceAddress,
                msg.sender,
                payloadHash,
                tokenSymbol,
                amount
            ) && expressCaller != address(0)
        ) _setExpressCallWithToken(slot, address(0));

        IExpressProxy(msg.sender).completeExecuteWithToken(
            expressCaller,
            commandId,
            sourceChain,
            sourceAddress,
            payload,
            tokenSymbol,
            amount
        );
    }

    /**
     * @dev Internal function used instead of a modifier to avoid the "stack too deep" error.
     * Checks that the caller is the correct ExpressProxy.
     */
    function _onlyProxy() internal view {
        address proxyRegistry = address(IExpressProxy(msg.sender).registry());

        if (msg.sender.codehash != proxyCodeHash || proxyRegistry != address(this)) revert NotExpressProxy();
    }

    function _getExpressCallWithToken(
        string calldata sourceChain,
        string calldata sourceAddress,
        address contractAddress,
        bytes32 payloadHash,
        string calldata symbol,
        uint256 amount
    ) internal view returns (bytes32 slot, address expressCaller) {
        slot = keccak256(abi.encode(sourceChain, sourceAddress, contractAddress, payloadHash, symbol, amount));
        expressCaller = expressCallsWithToken[slot];
    }

    function _setExpressCallWithToken(bytes32 slot, address expressCaller) internal {
        expressCallsWithToken[slot] = expressCaller;
    }
}
