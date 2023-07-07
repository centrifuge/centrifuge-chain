// SPDX-License-Identifier: MIT

pragma solidity ^0.8.0;

import { IExpressProxyDeployer } from '../interfaces/IExpressProxyDeployer.sol';
import { IExpressProxy } from '../interfaces/IExpressProxy.sol';
import { Create3 } from '../deploy/Create3.sol';
import { ExpressProxy } from './ExpressProxy.sol';
import { ExpressRegistry } from './ExpressRegistry.sol';

/**
 * @title ExpressProxyDeployer
 * @notice A contract for deploying instances of ExpressProxy using the create3 library for deterministic deployment.
 * This contract stores the codehash of the ExpressProxy and ExpressRegistry contracts and provides functions
 * to deploy new instances of ExpressProxy and to verify if a certain address corresponds to an express proxy.
 */
contract ExpressProxyDeployer is IExpressProxyDeployer {
    address public immutable gateway;
    bytes32 public immutable proxyCodeHash;
    bytes32 public immutable registryCodeHash;

    constructor(address gateway_) {
        if (gateway_ == address(0)) revert InvalidAddress();

        gateway = gateway_;

        ExpressProxy proxy = new ExpressProxy(address(1), address(1), '', gateway_);
        proxy.deployRegistry(type(ExpressRegistry).creationCode);

        proxyCodeHash = address(proxy).codehash;
        registryCodeHash = address(proxy.registry()).codehash;
    }

    /**
     * @notice Checks whether a given address corresponds to an express proxy.
     * @param proxyAddress The address to check.
     * @return boolean indicating whether the given address corresponds to an express proxy.
     */
    function isExpressProxy(address proxyAddress) external view returns (bool) {
        address expressRegistry = address(IExpressProxy(proxyAddress).registry());

        return proxyAddress.codehash == proxyCodeHash && expressRegistry.codehash == registryCodeHash;
    }

    /**
     * @notice Computes the address of an express proxy that was/will be deployed using a given salt and sender.
     * @param salt The salt used for the deployment.
     * @param sender The address performing the deployment.
     * @param host The contract delegating call to this contract.
     * @return address of the express proxy that was/will be deployed.
     */
    function deployedProxyAddress(
        bytes32 salt,
        address sender,
        address host
    ) external pure returns (address) {
        bytes32 deploySalt = keccak256(abi.encode(sender, salt));
        return Create3.deployedAddress(host, deploySalt);
    }

    /**
     * @notice Deploys an instance of ExpressProxy and the associated ExpressRegistry.
     * @dev delegatecall to this function to deploy a proxy from a host contract.
     * @dev payable is for compatibility to delegate from payable functions.
     * @param deploySalt The salt to use for the deployment.
     * @param implementationAddress The address of the implementation.
     * @param owner The owner of the proxy.
     * @param setupParams The parameters to pass to the setup function of the implementation.
     * @return proxyAddress address of the deployed proxy.
     */
    function deployExpressProxy(
        bytes32 deploySalt,
        address implementationAddress,
        address owner,
        bytes memory setupParams
    ) external payable returns (address proxyAddress) {
        proxyAddress = Create3.deploy(
            deploySalt,
            abi.encodePacked(
                type(ExpressProxy).creationCode,
                abi.encode(implementationAddress, owner, setupParams, gateway)
            )
        );

        ExpressProxy proxy = ExpressProxy(payable(proxyAddress));
        proxy.deployRegistry(type(ExpressRegistry).creationCode);
    }
}
