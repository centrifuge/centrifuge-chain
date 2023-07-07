// SPDX-License-Identifier: MIT

pragma solidity ^0.8.0;

interface IExpressProxyDeployer {
    error InvalidAddress();

    function gateway() external view returns (address);

    function proxyCodeHash() external view returns (bytes32);

    function registryCodeHash() external view returns (bytes32);

    function isExpressProxy(address proxyAddress) external view returns (bool);

    function deployedProxyAddress(
        bytes32 salt,
        address sender,
        address deployer
    ) external pure returns (address deployedAddress);

    function deployExpressProxy(
        bytes32 deploySalt,
        address implementationAddress,
        address owner,
        bytes memory setupParams
    ) external payable returns (address);
}
