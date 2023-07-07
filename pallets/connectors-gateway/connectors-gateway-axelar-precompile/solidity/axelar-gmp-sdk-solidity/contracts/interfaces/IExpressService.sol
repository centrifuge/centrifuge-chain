// SPDX-License-Identifier: MIT

pragma solidity ^0.8.18;

// This should be owned by the microservice that is paying for gas.
interface IExpressService {
    error InvalidOperator();
    error InvalidContractAddress();
    error InvalidTokenSymbol();
    error NotOperator();
    error NotExpressProxy();

    function expressOperator() external returns (address);

    function isExpressProxy(address proxyAddress) external view returns (bool);

    function deployedProxyAddress(bytes32 salt, address sender) external view returns (address deployedAddress);

    function deployExpressProxy(
        bytes32 salt,
        address implementationAddress,
        address owner,
        bytes calldata setupParams
    ) external returns (address);

    function callWithToken(
        bytes32 commandId,
        string calldata sourceChain,
        string calldata sourceAddress,
        address contractAddress,
        bytes calldata payload,
        string calldata tokenSymbol,
        uint256 amount
    ) external;

    function withdraw(
        address payable receiver,
        address token,
        uint256 amount
    ) external;
}
