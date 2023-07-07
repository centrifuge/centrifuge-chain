// SPDX-License-Identifier: MIT

pragma solidity ^0.8.0;

interface IGovernable {
    error NotGovernance();
    error NotMintLimiter();
    error InvalidGovernance();
    error InvalidMintLimiter();

    event GovernanceTransferred(address indexed previousGovernance, address indexed newGovernance);
    event MintLimiterTransferred(address indexed previousGovernance, address indexed newGovernance);

    function governance() external view returns (address);

    function mintLimiter() external view returns (address);

    function transferGovernance(address newGovernance) external;

    function transferMintLimiter(address newGovernance) external;
}
