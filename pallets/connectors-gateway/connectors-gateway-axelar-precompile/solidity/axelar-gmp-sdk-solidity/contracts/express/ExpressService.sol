// SPDX-License-Identifier: MIT

pragma solidity ^0.8.18;

import { Upgradable } from '../upgradable/Upgradable.sol';
import { SafeTokenTransfer, SafeNativeTransfer } from '../utils/SafeTransfer.sol';
import { IExpressProxy } from '../interfaces/IExpressProxy.sol';
import { IERC20 } from '../interfaces/IERC20.sol';
import { IExpressProxyDeployer } from '../interfaces/IExpressProxyDeployer.sol';
import { IExpressService } from '../interfaces/IExpressService.sol';
import { AxelarExecutable } from '../executable/AxelarExecutable.sol';

/**
 * @title ExpressService
 * @notice A contract for facilitating express service operations in the Axelar network.
 * @dev It inherits the Upgradable and AxelarExecutable contracts, and implements the IExpressService interface.
 */
contract ExpressService is Upgradable, AxelarExecutable, IExpressService {
    using SafeTokenTransfer for IERC20;
    using SafeNativeTransfer for address payable;

    IExpressProxyDeployer public immutable proxyDeployer;

    address public immutable expressOperator;

    /**
     * @notice Constructor for creating a new ExpressService contract.
     * @param gateway_ The instance of the AxelarGateway contract.
     * @param proxyDeployer_ The instance of the ExpressProxyDeployer contract.
     * @param expressOperator_ The address of the express operator.
     */
    constructor(
        address gateway_,
        address proxyDeployer_,
        address expressOperator_
    ) AxelarExecutable(gateway_) {
        if (expressOperator_ == address(0)) revert InvalidOperator();
        if (proxyDeployer_ == address(0)) revert InvalidAddress();

        proxyDeployer = IExpressProxyDeployer(proxyDeployer_);
        expressOperator = expressOperator_;
    }

    /**
     * @dev Modifier to restrict function access to the express operator.
     */
    modifier onlyOperator() {
        if (msg.sender != expressOperator) revert NotOperator();

        _;
    }

    /**
     * @notice Checks if an address is an express proxy.
     * @param proxyAddress The address to check.
     * @return bool indicating whether the address is an express proxy.
     */
    function isExpressProxy(address proxyAddress) public view returns (bool) {
        return proxyDeployer.isExpressProxy(proxyAddress);
    }

    /**
     * @notice Returns the deployed proxy address.
     * @param salt The salt used for deploying the proxy.
     * @param sender The address used to deploy the proxy.
     * @return address of the deployed proxy.
     */
    function deployedProxyAddress(bytes32 salt, address sender) external view returns (address) {
        return proxyDeployer.deployedProxyAddress(salt, sender, address(this));
    }

    /**
     * @notice Deploys an express proxy.
     * @param salt The salt used for deploying the proxy.
     * @param implementationAddress The address of the ExpressExecutable implementation.
     * @param owner The owner of the deployed proxy.
     * @param setupParams The parameters used for setting up the implementation.
     * @return deployedAddress address of the deployed proxy.
     */
    function deployExpressProxy(
        bytes32 salt,
        address implementationAddress,
        address owner,
        bytes calldata setupParams
    ) external returns (address deployedAddress) {
        bytes32 deploySalt = keccak256(abi.encode(msg.sender, salt));
        (, bytes memory data) = address(proxyDeployer).delegatecall(
            abi.encodeWithSelector(
                IExpressProxyDeployer.deployExpressProxy.selector,
                deploySalt,
                implementationAddress,
                owner,
                setupParams
            )
        );
        (deployedAddress) = abi.decode(data, (address));
    }

    /**
     * @notice Executes a call with a token, if the command has not be executed by the AxelarGateway performs
     * an express execute with token. If the command has been executed by the AxelarGateway, performs an execute
     * with token.
     * @param commandId The ID of the command.
     * @param sourceChain The source chain of the call.
     * @param sourceAddress The source address of the call.
     * @param contractAddress The contract address for the call.
     * @param payload The payload of the call.
     * @param tokenSymbol The symbol of the token associated with the call.
     * @param amount The amount of the token associated with the call.
     */
    function callWithToken(
        bytes32 commandId,
        string calldata sourceChain,
        string calldata sourceAddress,
        address contractAddress,
        bytes calldata payload,
        string calldata tokenSymbol,
        uint256 amount
    ) external onlyOperator {
        if (contractAddress == address(0)) revert InvalidContractAddress();

        if (commandId != bytes32(0) && gateway.isCommandExecuted(commandId)) {
            IExpressProxy(contractAddress).executeWithToken(
                commandId,
                sourceChain,
                sourceAddress,
                payload,
                tokenSymbol,
                amount
            );
        } else {
            if (!isExpressProxy(contractAddress)) revert NotExpressProxy();

            address tokenAddress = gateway.tokenAddresses(tokenSymbol);

            if (tokenAddress == address(0)) revert InvalidTokenSymbol();

            IERC20(tokenAddress).approve(contractAddress, amount);
            IExpressProxy(contractAddress).expressExecuteWithToken(
                sourceChain,
                sourceAddress,
                payload,
                tokenSymbol,
                amount
            );
        }
    }

    /**
     * @notice Withdraws tokens to a receiver.
     * @param receiver The receiver of the withdrawal.
     * @param token The token to withdraw.
     * @param amount The amount of tokens to withdraw.
     */
    function withdraw(
        address payable receiver,
        address token,
        uint256 amount
    ) external onlyOperator {
        if (receiver == address(0)) revert InvalidAddress();

        if (token == address(0)) {
            receiver.safeNativeTransfer(amount);
        } else {
            IERC20(token).safeTransfer(receiver, amount);
        }
    }

    /**
     * @notice Returns the ID of the contract.
     * @return bytes32 ID of the contract.
     */
    function contractId() external pure returns (bytes32) {
        return keccak256('axelar-gmp-express-service');
    }
}
