// SPDX-License-Identifier: MIT

pragma solidity 0.8.9;

import { SafeTokenTransfer, SafeNativeTransfer } from '@axelar-network/axelar-gmp-sdk-solidity/contracts/utils/SafeTransfer.sol';
import { IERC20 } from '@axelar-network/axelar-gmp-sdk-solidity/contracts/interfaces/IERC20.sol';
import { IAxelarDepositService } from '../interfaces/IAxelarDepositService.sol';
import { IAxelarGateway } from '../interfaces/IAxelarGateway.sol';
import { IWETH9 } from '../interfaces/IWETH9.sol';
import { Upgradable } from '../util/Upgradable.sol';
import { DepositServiceBase } from './DepositServiceBase.sol';
import { DepositReceiver } from './DepositReceiver.sol';
import { ReceiverImplementation } from './ReceiverImplementation.sol';

// This should be owned by the microservice that is paying for gas.
contract AxelarDepositService is Upgradable, DepositServiceBase, IAxelarDepositService {
    using SafeTokenTransfer for IERC20;
    using SafeNativeTransfer for address;

    // This public storage for ERC20 token intended to be refunded.
    // It triggers the DepositReceiver/ReceiverImplementation to switch into a refund mode.
    // Address is stored and deleted withing the same refund transaction.
    address public refundToken;

    address public immutable receiverImplementation;
    address public immutable refundIssuer;

    constructor(
        address gateway_,
        string memory wrappedSymbol_,
        address refundIssuer_
    ) DepositServiceBase(gateway_, wrappedSymbol_) {
        if (refundIssuer_ == address(0)) revert InvalidAddress();

        refundIssuer = refundIssuer_;
        receiverImplementation = address(new ReceiverImplementation(gateway_, wrappedSymbol_));
    }

    // @dev This method is meant to be called directly by user to send native token cross-chain
    function sendNative(string calldata destinationChain, string calldata destinationAddress) external payable {
        address wrappedTokenAddress = wrappedToken();
        uint256 amount = msg.value;

        if (amount == 0) revert NothingDeposited();

        // Wrapping the native currency and into WETH-like token
        IWETH9(wrappedTokenAddress).deposit{ value: amount }();
        // Not doing safe approval as gateway will revert anyway if approval fails
        // We expect allowance to always be 0 at this point
        IWETH9(wrappedTokenAddress).approve(gateway, amount);
        // Sending the token trough the gateway
        IAxelarGateway(gateway).sendToken(destinationChain, destinationAddress, wrappedSymbol(), amount);
    }

    // @dev Generates a deposit address for sending an ERC20 token cross-chain
    function addressForTokenDeposit(
        bytes32 salt,
        address refundAddress,
        string calldata destinationChain,
        string calldata destinationAddress,
        string calldata tokenSymbol
    ) external view returns (address) {
        return
            _depositAddress(
                salt,
                abi.encodeWithSelector(
                    ReceiverImplementation.receiveAndSendToken.selector,
                    refundAddress,
                    destinationChain,
                    destinationAddress,
                    tokenSymbol
                ),
                refundAddress
            );
    }

    // @dev Generates a deposit address for sending native currency cross-chain
    function addressForNativeDeposit(
        bytes32 salt,
        address refundAddress,
        string calldata destinationChain,
        string calldata destinationAddress
    ) public view returns (address) {
        return
            _depositAddress(
                salt,
                abi.encodeWithSelector(
                    ReceiverImplementation.receiveAndSendNative.selector,
                    refundAddress,
                    destinationChain,
                    destinationAddress
                ),
                refundAddress
            );
    }

    // @dev Generates a deposit address for unwrapping WETH-like token into native currency
    function addressForNativeUnwrap(
        bytes32 salt,
        address refundAddress,
        address recipient
    ) external view returns (address) {
        return
            _depositAddress(
                salt,
                abi.encodeWithSelector(ReceiverImplementation.receiveAndUnwrapNative.selector, refundAddress, recipient),
                refundAddress
            );
    }

    // @dev Receives ERC20 token from the deposit address and sends it cross-chain
    function sendTokenDeposit(
        bytes32 salt,
        address refundAddress,
        string calldata destinationChain,
        string calldata destinationAddress,
        string calldata tokenSymbol
    ) external {
        // NOTE: `DepositReceiver` is destroyed in the same runtime context that it is deployed.
        new DepositReceiver{ salt: salt }(
            abi.encodeWithSelector(
                ReceiverImplementation.receiveAndSendToken.selector,
                refundAddress,
                destinationChain,
                destinationAddress,
                tokenSymbol
            ),
            refundAddress
        );
    }

    // @dev Refunds ERC20 tokens from the deposit address if they don't match the intended token
    // Only refundAddress can refund the token that was intended to go cross-chain (if not sent yet)
    function refundTokenDeposit(
        bytes32 salt,
        address refundAddress,
        string calldata destinationChain,
        string calldata destinationAddress,
        string calldata tokenSymbol,
        address[] calldata refundTokens
    ) external {
        address intendedToken = IAxelarGateway(gateway).tokenAddresses(tokenSymbol);

        uint256 tokensLength = refundTokens.length;
        for (uint256 i; i < tokensLength; ++i) {
            // Allowing only the refundAddress to refund the intended token
            if (refundTokens[i] == intendedToken && msg.sender != refundAddress) continue;

            // Saving to public storage to be accessed by the DepositReceiver
            refundToken = refundTokens[i];
            // NOTE: `DepositReceiver` is destroyed in the same runtime context that it is deployed.
            new DepositReceiver{ salt: salt }(
                abi.encodeWithSelector(
                    ReceiverImplementation.receiveAndSendToken.selector,
                    refundAddress,
                    destinationChain,
                    destinationAddress,
                    tokenSymbol
                ),
                refundAddress
            );
        }

        refundToken = address(0);
    }

    // @dev Receives native currency, wraps it into WETH-like token and sends cross-chain
    function sendNativeDeposit(
        bytes32 salt,
        address refundAddress,
        string calldata destinationChain,
        string calldata destinationAddress
    ) external {
        // NOTE: `DepositReceiver` is destroyed in the same runtime context that it is deployed.
        new DepositReceiver{ salt: salt }(
            abi.encodeWithSelector(
                ReceiverImplementation.receiveAndSendNative.selector,
                refundAddress,
                destinationChain,
                destinationAddress
            ),
            refundAddress
        );
    }

    // @dev Refunds ERC20 tokens from the deposit address after the native deposit was sent
    // Only refundAddress can refund the native currency intended to go cross-chain (if not sent yet)
    function refundNativeDeposit(
        bytes32 salt,
        address refundAddress,
        string calldata destinationChain,
        string calldata destinationAddress,
        address[] calldata refundTokens
    ) external {
        // Allowing only the refundAddress to refund the native currency
        if (addressForNativeDeposit(salt, refundAddress, destinationChain, destinationAddress).balance > 0 && msg.sender != refundAddress)
            return;

        uint256 tokensLength = refundTokens.length;
        for (uint256 i; i < tokensLength; ++i) {
            refundToken = refundTokens[i];
            // NOTE: `DepositReceiver` is destroyed in the same runtime context that it is deployed.
            new DepositReceiver{ salt: salt }(
                abi.encodeWithSelector(
                    ReceiverImplementation.receiveAndSendNative.selector,
                    refundAddress,
                    destinationChain,
                    destinationAddress
                ),
                refundAddress
            );
        }

        refundToken = address(0);
    }

    // @dev Receives WETH-like token, unwraps and send native currency to the recipient
    function nativeUnwrap(
        bytes32 salt,
        address refundAddress,
        address payable recipient
    ) external {
        // NOTE: `DepositReceiver` is destroyed in the same runtime context that it is deployed.
        new DepositReceiver{ salt: salt }(
            abi.encodeWithSelector(ReceiverImplementation.receiveAndUnwrapNative.selector, refundAddress, recipient),
            refundAddress
        );
    }

    // @dev Refunds ERC20 tokens from the deposit address except WETH-like token
    // Only refundAddress can refund the WETH-like token intended to be unwrapped (if not yet)
    function refundNativeUnwrap(
        bytes32 salt,
        address refundAddress,
        address payable recipient,
        address[] calldata refundTokens
    ) external {
        address wrappedTokenAddress = wrappedToken();

        uint256 tokensLength = refundTokens.length;
        for (uint256 i; i < tokensLength; ++i) {
            // Allowing only the refundAddress to refund the intended WETH-like token
            if (refundTokens[i] == wrappedTokenAddress && msg.sender != refundAddress) continue;

            refundToken = refundTokens[i];
            // NOTE: `DepositReceiver` is destroyed in the same runtime context that it is deployed.
            new DepositReceiver{ salt: salt }(
                abi.encodeWithSelector(ReceiverImplementation.receiveAndUnwrapNative.selector, refundAddress, recipient),
                refundAddress
            );
        }

        refundToken = address(0);
    }

    function refundLockedAsset(
        address receiver,
        address token,
        uint256 amount
    ) external {
        if (msg.sender != refundIssuer) revert NotRefundIssuer();
        if (receiver == address(0)) revert InvalidAddress();
        if (amount == 0) revert InvalidAmount();

        if (token == address(0)) {
            receiver.safeNativeTransfer(amount);
        } else {
            IERC20(token).safeTransfer(receiver, amount);
        }
    }

    function _depositAddress(
        bytes32 salt,
        bytes memory delegateData,
        address refundAddress
    ) internal view returns (address) {
        /* Convert a hash which is bytes32 to an address which is 20-byte long
        according to https://docs.soliditylang.org/en/v0.8.9/control-structures.html?highlight=create2#salted-contract-creations-create2 */
        return
            address(
                uint160(
                    uint256(
                        keccak256(
                            abi.encodePacked(
                                hex'ff',
                                address(this),
                                salt,
                                // Encoding delegateData and refundAddress as constructor params
                                keccak256(abi.encodePacked(type(DepositReceiver).creationCode, abi.encode(delegateData, refundAddress)))
                            )
                        )
                    )
                )
            );
    }

    function contractId() external pure returns (bytes32) {
        return keccak256('axelar-deposit-service');
    }
}
