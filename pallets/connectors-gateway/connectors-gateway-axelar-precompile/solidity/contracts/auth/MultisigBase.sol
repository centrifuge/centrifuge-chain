// SPDX-License-Identifier: MIT

pragma solidity ^0.8.0;

import { IMultisigBase } from '../interfaces/IMultisigBase.sol';

/**
 * @title MultisigBase Contract
 * @notice This contract implements a custom multisignature wallet where transactions must be confirmed by a
 * threshold of signers. The signers and threshold may be updated every `epoch`.
 */
contract MultisigBase is IMultisigBase {
    struct Voting {
        uint256 voteCount;
        mapping(address => bool) hasVoted;
    }

    struct Signers {
        address[] accounts;
        uint256 threshold;
        mapping(address => bool) isSigner;
    }

    Signers public signers;
    uint256 public signerEpoch;
    // uint256 is for epoch, bytes32 for vote topic hash
    mapping(uint256 => mapping(bytes32 => Voting)) public votingPerTopic;

    /**
     * @notice Contract constructor
     * @dev Sets the initial list of signers and corresponding threshold.
     * @param accounts Address array of the signers
     * @param threshold Signature threshold required to validate a transaction
     */
    constructor(address[] memory accounts, uint256 threshold) {
        _rotateSigners(accounts, threshold);
    }

    /**
     * @notice Modifier to ensure the caller is a signer
     * @dev Keeps track of votes for each operation and resets the vote count if the operation is executed.
     * @dev Given the early void return, this modifier should be used with care on functions that return data.
     */
    modifier onlySigners() {
        if (!signers.isSigner[msg.sender]) revert NotSigner();

        bytes32 topic = keccak256(msg.data);
        Voting storage voting = votingPerTopic[signerEpoch][topic];

        // Check that signer has not voted, then record that they have voted.
        if (voting.hasVoted[msg.sender]) revert AlreadyVoted();

        voting.hasVoted[msg.sender] = true;

        // Determine the new vote count.
        uint256 voteCount = voting.voteCount + 1;

        // Do not proceed with operation execution if insufficient votes.
        if (voteCount < signers.threshold) {
            // Save updated vote count.
            voting.voteCount = voteCount;
            return;
        }

        // Clear vote count and voted booleans.
        voting.voteCount = 0;

        uint256 count = signers.accounts.length;

        for (uint256 i; i < count; ++i) {
            voting.hasVoted[signers.accounts[i]] = false;
        }

        emit MultisigOperationExecuted(topic);

        _;
    }

    /******************\
    |* Public Getters *|
    \******************/

    /**
     * @notice Returns the current signer threshold
     * @return uint The signer threshold
     */
    function signerThreshold() external view override returns (uint256) {
        return signers.threshold;
    }

    /**
     * @notice Returns an array of current signers
     * @return array of signer addresses
     */
    function signerAccounts() external view override returns (address[] memory) {
        return signers.accounts;
    }

    /***********\
    |* Setters *|
    \***********/

    /**
     * @notice Rotate the signers for the multisig
     * @dev Updates the current set of signers and threshold and increments the `epoch`
     * @dev This function is protected by the onlySigners modifier
     * @param newAccounts Address array of the new signers
     * @param newThreshold The new signature threshold for executing operations
     */
    function rotateSigners(address[] memory newAccounts, uint256 newThreshold) external virtual onlySigners {
        _rotateSigners(newAccounts, newThreshold);
    }

    /**
     * @dev Internal function that implements signer rotation logic
     */
    function _rotateSigners(address[] memory newAccounts, uint256 newThreshold) internal {
        uint256 length = signers.accounts.length;

        // Clean up old signers.
        for (uint256 i; i < length; ++i) {
            signers.isSigner[signers.accounts[i]] = false;
        }

        length = newAccounts.length;

        if (newThreshold > length) revert InvalidSigners();

        if (newThreshold == 0) revert InvalidSignerThreshold();

        ++signerEpoch;

        signers.accounts = newAccounts;
        signers.threshold = newThreshold;

        for (uint256 i; i < length; ++i) {
            address account = newAccounts[i];

            // Check that the account wasn't already set as a signer for this epoch.
            if (signers.isSigner[account]) revert DuplicateSigner(account);
            if (account == address(0)) revert InvalidSigners();

            signers.isSigner[account] = true;
        }

        emit SignersRotated(newAccounts, newThreshold);
    }
}
