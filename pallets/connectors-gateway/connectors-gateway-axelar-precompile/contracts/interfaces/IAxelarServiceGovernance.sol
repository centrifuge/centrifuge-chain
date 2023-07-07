// SPDX-License-Identifier: MIT

pragma solidity ^0.8.0;

import { IInterchainGovernance } from './IInterchainGovernance.sol';
import { IMultisigBase } from './IMultisigBase.sol';

/**
 * @title IAxelarServiceGovernance Interface
 * @dev This interface extends IInterchainGovernance and IMultisigBase for multisig proposal actions
 */
interface IAxelarServiceGovernance is IMultisigBase, IInterchainGovernance {
    error NotApproved();

    event MultisigApproved(bytes32 indexed proposalHash, address indexed targetContract, bytes callData, uint256 nativeValue);
    event MultisigCancelled(bytes32 indexed proposalHash, address indexed targetContract, bytes callData, uint256 nativeValue);
    event MultisigExecuted(bytes32 indexed proposalHash, address indexed targetContract, bytes callData, uint256 nativeValue);

    /**
     * @notice Executes a multisig proposal
     * @param targetContract The target address the proposal will call
     * @param callData The data that encodes the function and arguments to call on the target contract
     */
    function executeMultisigProposal(
        address targetContract,
        bytes calldata callData,
        uint256 value
    ) external payable;
}
