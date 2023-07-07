// SPDX-License-Identifier: MIT

pragma solidity ^0.8.0;

import { AxelarExecutable } from '@axelar-network/axelar-gmp-sdk-solidity/contracts/executable/AxelarExecutable.sol';
import { TimeLock } from '@axelar-network/axelar-gmp-sdk-solidity/contracts/utils/TimeLock.sol';
import { IInterchainGovernance } from '../interfaces/IInterchainGovernance.sol';
import { Caller } from '../util/Caller.sol';

/**
 * @title Interchain Governance contract
 * @notice This contract handles cross-chain governance actions. It includes functionality
 * to create, cancel, and execute governance proposals.
 */
contract InterchainGovernance is AxelarExecutable, TimeLock, Caller, IInterchainGovernance {
    enum GovernanceCommand {
        ScheduleTimeLockProposal,
        CancelTimeLockProposal
    }

    string public governanceChain;
    string public governanceAddress;
    bytes32 public immutable governanceChainHash;
    bytes32 public immutable governanceAddressHash;

    /**
     * @notice Initializes the contract
     * @param gateway The address of the Axelar gateway contract
     * @param governanceChain_ The name of the governance chain
     * @param governanceAddress_ The address of the governance contract
     * @param minimumTimeDelay The minimum time delay for timelock operations
     */
    constructor(
        address gateway,
        string memory governanceChain_,
        string memory governanceAddress_,
        uint256 minimumTimeDelay
    ) AxelarExecutable(gateway) TimeLock(minimumTimeDelay) {
        governanceChain = governanceChain_;
        governanceAddress = governanceAddress_;
        governanceChainHash = keccak256(bytes(governanceChain_));
        governanceAddressHash = keccak256(bytes(governanceAddress_));
    }

    /**
     * @notice Returns the ETA of a proposal
     * @param target The address of the contract targeted by the proposal
     * @param callData The call data to be sent to the target contract
     * @param nativeValue The amount of native tokens to be sent to the target contract
     * @return uint256 The ETA of the proposal
     */
    function getProposalEta(
        address target,
        bytes calldata callData,
        uint256 nativeValue
    ) external view returns (uint256) {
        return _getTimeLockEta(_getProposalHash(target, callData, nativeValue));
    }

    /**
     * @notice Executes a proposal
     * @dev The proposal is executed by calling the target contract with calldata. Native value is
     * transferred with the call to the target contract.
     * @param target The target address of the contract to call
     * @param callData The data containing the function and arguments for the contract to call
     * @param nativeValue The amount of native token to send to the target contract
     */
    function executeProposal(
        address target,
        bytes calldata callData,
        uint256 nativeValue
    ) external payable {
        bytes32 proposalHash = keccak256(abi.encodePacked(target, callData, nativeValue));

        _finalizeTimeLock(proposalHash);
        _call(target, callData, nativeValue);

        emit ProposalExecuted(proposalHash, target, callData, nativeValue, block.timestamp);
    }

    /**
     * @notice Internal function to execute a proposal action
     * @param sourceChain The source chain of the proposal, must equal the governance chain
     * @param sourceAddress The source address of the proposal, must equal the governance address
     * @param payload The payload of the proposal
     */
    function _execute(
        string calldata sourceChain,
        string calldata sourceAddress,
        bytes calldata payload
    ) internal override {
        if (keccak256(bytes(sourceChain)) != governanceChainHash || keccak256(bytes(sourceAddress)) != governanceAddressHash)
            revert NotGovernance();

        (uint256 command, address target, bytes memory callData, uint256 nativeValue, uint256 eta) = abi.decode(
            payload,
            (uint256, address, bytes, uint256, uint256)
        );

        if (target == address(0)) revert InvalidTarget();

        _processCommand(command, target, callData, nativeValue, eta);
    }

    /**
     * @notice Internal function to process a governance command
     * @param commandId The id of the command, 0 for proposal creation and 1 for proposal cancellation
     * @param target The target address the proposal will call
     * @param callData The data the encodes the function and arguments to call on the target contract
     * @param nativeValue The nativeValue of native token to be sent to the target contract
     * @param eta The time after which the proposal can be executed
     */
    function _processCommand(
        uint256 commandId,
        address target,
        bytes memory callData,
        uint256 nativeValue,
        uint256 eta
    ) internal virtual {
        if (commandId > uint256(type(GovernanceCommand).max)) {
            revert InvalidCommand();
        }

        GovernanceCommand command = GovernanceCommand(commandId);
        bytes32 proposalHash = _getProposalHash(target, callData, nativeValue);

        if (command == GovernanceCommand.ScheduleTimeLockProposal) {
            eta = _scheduleTimeLock(proposalHash, eta);

            emit ProposalScheduled(proposalHash, target, callData, nativeValue, eta);
            return;
        } else if (command == GovernanceCommand.CancelTimeLockProposal) {
            _cancelTimeLock(proposalHash);

            emit ProposalCancelled(proposalHash, target, callData, nativeValue, eta);
            return;
        }
    }

    /**
     * @dev Get proposal hash using the target, callData, and nativeValue
     */
    function _getProposalHash(
        address target,
        bytes memory callData,
        uint256 nativeValue
    ) internal pure returns (bytes32) {
        return keccak256(abi.encodePacked(target, callData, nativeValue));
    }

    /**
     * @notice Overrides internal function of AxelarExecutable, will always revert
     * as this governance module does not support execute with token.
     */
    function _executeWithToken(
        string calldata, /* sourceChain */
        string calldata, /* sourceAddress */
        bytes calldata, /* payload */
        string calldata, /* tokenSymbol */
        uint256 /* amount */
    ) internal pure override {
        revert TokenNotSupported();
    }

    /**
     * @notice Making contact able to receive native value
     */
    receive() external payable {}
}
