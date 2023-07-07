// SPDX-License-Identifier: MIT

pragma solidity ^0.8.0;

import { IERC20 } from '../interfaces/IERC20.sol';
import { IAxelarGateway } from '../interfaces/IAxelarGateway.sol';
import { IExpressProxy } from '../interfaces/IExpressProxy.sol';
import { IExpressService } from '../interfaces/IExpressService.sol';
import { IExpressRegistry } from '../interfaces/IExpressRegistry.sol';
import { IExpressExecutable } from '../interfaces/IExpressExecutable.sol';
import { FinalProxy } from '../upgradable/FinalProxy.sol';
import { SafeTokenTransfer, SafeTokenTransferFrom } from '../utils/SafeTransfer.sol';
import { Create3 } from '../deploy/Create3.sol';

/**
 * @title ExpressProxy
 * @notice A special type of proxy contract used for ExpressExecutable contracts.
 * @dev It extends the FinalProxy contract and implements the IExpressProxy interface.
 * It utilizes safe transfer functionalities from the SafeTokenTransfer library for IERC20 tokens.
 * It interacts with an ExpressRegistry contract to keep track of GMP Express calls.
 */
contract ExpressProxy is FinalProxy, IExpressProxy {
    using SafeTokenTransfer for IERC20;
    using SafeTokenTransferFrom for IERC20;

    bytes32 internal constant REGISTRY_SALT = keccak256('express-registry');

    IAxelarGateway public immutable gateway;

    /**
     * @notice Constructor for creating a new ExpressProxy contract.
     * @param implementationAddress The address of the implementation contract.
     * @param owner The owner of the proxy.
     * @param setupParams The parameters for setting up the implementation.
     * @param gateway_ The instance of the AxelarGateway contract.
     */
    constructor(
        address implementationAddress,
        address owner,
        bytes memory setupParams,
        address gateway_
    ) FinalProxy(implementationAddress, owner, setupParams) {
        if (gateway_ == address(0)) revert InvalidAddress();

        gateway = IAxelarGateway(gateway_);
    }

    /**
     * @dev A modifier that ensures only the ExpressRegistry can call the function.
     */
    modifier onlyRegistry() {
        if (msg.sender != address(registry())) revert NotExpressRegistry();

        _;
    }

    /**
     * @notice Returns the ExpressRegistry associated with this proxy.
     * @return address of the corresponding ExpressRegistry contract.
     */
    function registry() public view returns (IExpressRegistry) {
        // Computing address is cheaper than using storage
        // Can't use immutable storage as it will alter the codehash for each proxy instance
        return IExpressRegistry(Create3.deployedAddress(address(this), REGISTRY_SALT));
    }

    /**
     * @notice Deploys the ExpressRegistry associated with this proxy.
     * @notice should be called right after the proxy is deployed.
     * @param registryCreationCode The creation code of the registry.
     */
    function deployRegistry(bytes calldata registryCreationCode) external {
        Create3.deploy(
            REGISTRY_SALT,
            abi.encodePacked(registryCreationCode, abi.encode(address(gateway), address(this)))
        );
    }

    /**
     * @notice Executes a command after validating the contract call with the AxelarGateway.
     * @param commandId The ID of the command to execute.
     * @param sourceChain The source chain of the command.
     * @param sourceAddress The source address of the command.
     * @param payload The payload of the command.
     */
    function execute(
        bytes32 commandId,
        string calldata sourceChain,
        string calldata sourceAddress,
        bytes calldata payload
    ) external override {
        bytes32 payloadHash = keccak256(payload);

        if (!gateway.validateContractCall(commandId, sourceChain, sourceAddress, payloadHash))
            revert NotApprovedByGateway();

        _execute(sourceChain, sourceAddress, payload);
    }

    /**
     * @notice Executes an express call with token.
     * @dev Validates that express calls are enabled by the Express Executable contract.
     * @param sourceChain The source chain of the command.
     * @param sourceAddress The source address of the command.
     * @param payload The payload of the command.
     * @param tokenSymbol The symbol of the token associated with the command.
     * @param amount The amount of the token associated with the command.
     */
    function expressExecuteWithToken(
        string calldata sourceChain,
        string calldata sourceAddress,
        bytes calldata payload,
        string calldata tokenSymbol,
        uint256 amount
    ) external override {
        bytes32 payloadHash = keccak256(payload);
        address token = gateway.tokenAddresses(tokenSymbol);

        if (
            !IExpressExecutable(address(this)).acceptExpressCallWithToken(
                msg.sender,
                sourceChain,
                sourceAddress,
                payloadHash,
                tokenSymbol,
                amount
            )
        ) revert ExpressCallNotAccepted();

        if (token == address(0)) revert InvalidTokenSymbol();

        registry().registerExpressCallWithToken(
            msg.sender,
            sourceChain,
            sourceAddress,
            payloadHash,
            tokenSymbol,
            amount
        );

        IERC20(token).safeTransferFrom(msg.sender, address(this), amount);
        _executeWithToken(sourceChain, sourceAddress, payload, tokenSymbol, amount);
    }

    /**
     * @notice Handles a normal GMP call when it arrives.
     * @param commandId The ID of the command to execute.
     * @param sourceChain The source chain of the command.
     * @param sourceAddress The source address of the command.
     * @param payload The payload of the command.
     * @param tokenSymbol The symbol of the token associated with the command.
     * @param amount The amount of the token associated with the command.
     */
    function executeWithToken(
        bytes32 commandId,
        string calldata sourceChain,
        string calldata sourceAddress,
        bytes calldata payload,
        string calldata tokenSymbol,
        uint256 amount
    ) external override {
        registry().processExecuteWithToken(commandId, sourceChain, sourceAddress, payload, tokenSymbol, amount);
    }

    /**
     * @notice Callback to complete the GMP call. Can only be called by the Express Registry.
     * @dev Called by the Express Registry in processExecuteWithToken.
     * @param expressCaller The address of the express caller.
     * @param commandId The ID of the command to execute.
     * @param sourceChain The source chain of the command.
     * @param sourceAddress The source address of the command.
     * @param payload The payload of the command.
     * @param tokenSymbol The symbol of the token associated with the command.
     * @param amount The amount of the token associated with the command.
     */
    function completeExecuteWithToken(
        address expressCaller,
        bytes32 commandId,
        string calldata sourceChain,
        string calldata sourceAddress,
        bytes calldata payload,
        string calldata tokenSymbol,
        uint256 amount
    ) external override onlyRegistry {
        bytes32 payloadHash = keccak256(payload);

        if (
            !gateway.validateContractCallAndMint(
                commandId,
                sourceChain,
                sourceAddress,
                payloadHash,
                tokenSymbol,
                amount
            )
        ) revert NotApprovedByGateway();

        if (expressCaller == address(0)) {
            _executeWithToken(sourceChain, sourceAddress, payload, tokenSymbol, amount);
        } else {
            // Returning the lent token
            address token = gateway.tokenAddresses(tokenSymbol);

            if (token == address(0)) revert InvalidTokenSymbol();

            IERC20(token).safeTransfer(expressCaller, amount);
        }
    }

    // Doing internal call to the implementation
    function _execute(
        string calldata sourceChain,
        string calldata sourceAddress,
        bytes calldata payload
    ) internal {
        (bool success, ) = implementation().delegatecall(
            abi.encodeWithSelector(ExpressProxy.execute.selector, bytes32(0), sourceChain, sourceAddress, payload)
        );

        // if not success revert with the original revert data
        if (!success) {
            assembly {
                let ptr := mload(0x40)
                let size := returndatasize()
                returndatacopy(ptr, 0, size)
                revert(ptr, size)
            }
        }
    }

    // Doing internal call to the implementation
    function _executeWithToken(
        string calldata sourceChain,
        string calldata sourceAddress,
        bytes calldata payload,
        string calldata tokenSymbol,
        uint256 amount
    ) internal {
        (bool success, ) = implementation().delegatecall(
            abi.encodeWithSelector(
                ExpressProxy.executeWithToken.selector,
                bytes32(0),
                sourceChain,
                sourceAddress,
                payload,
                tokenSymbol,
                amount
            )
        );

        // if not success revert with the original revert data
        if (!success) {
            assembly {
                let ptr := mload(0x40)
                let size := returndatasize()
                returndatacopy(ptr, 0, size)
                revert(ptr, size)
            }
        }
    }
}
