// SPDX-License-Identifier: MIT

pragma solidity ^0.8.0;

import { IAxelarExecutable } from '@axelar-network/axelar-gmp-sdk-solidity/contracts/interfaces/IAxelarExecutable.sol';
import { ICaller } from './ICaller.sol';

/**
 * @title IInterchainGovernance Interface
 * @notice This interface extends IAxelarExecutable for interchain governance mechanisms.
 */
interface IInterchainGovernance is IAxelarExecutable, ICaller {
    error NotGovernance();
    error InvalidCommand();
    error InvalidTarget();
    error TokenNotSupported();

    event ProposalScheduled(bytes32 indexed proposalHash, address indexed target, bytes callData, uint256 value, uint256 indexed eta);
    event ProposalCancelled(bytes32 indexed proposalHash, address indexed target, bytes callData, uint256 value, uint256 indexed eta);
    event ProposalExecuted(bytes32 indexed proposalHash, address indexed target, bytes callData, uint256 value, uint256 indexed timestamp);

    /**
     * @notice Returns the name of the governance chain.
     * @return string The name of the governance chain
     */
    function governanceChain() external view returns (string memory);

    /**
     * @notice Returns the address of the governance address.
     * @return string The address of the governance address
     */
    function governanceAddress() external view returns (string memory);

    /**
     * @notice Returns the hash of the governance chain.
     * @return bytes32 The hash of the governance chain
     */
    function governanceChainHash() external view returns (bytes32);

    /**
     * @notice Returns the hash of the governance address.
     * @return bytes32 The hash of the governance address
     */
    function governanceAddressHash() external view returns (bytes32);

    /**
     * @notice Returns the ETA of a proposal.
     * @param target The address of the contract targeted by the proposal
     * @param callData The call data to be sent to the target contract
     * @param nativeValue The amount of native tokens to be sent to the target contract
     * @return uint256 The ETA of the proposal
     */
    function getProposalEta(
        address target,
        bytes calldata callData,
        uint256 nativeValue
    ) external view returns (uint256);

    /**
     * @notice Executes a governance proposal.
     * @param targetContract The address of the contract targeted by the proposal
     * @param callData The call data to be sent to the target contract
     * @param value The amount of ETH to be sent to the target contract
     */
    function executeProposal(
        address targetContract,
        bytes calldata callData,
        uint256 value
    ) external payable;
}
