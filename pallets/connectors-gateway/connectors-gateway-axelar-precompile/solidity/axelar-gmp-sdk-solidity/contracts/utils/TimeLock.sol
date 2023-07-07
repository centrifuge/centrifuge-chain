// SPDX-License-Identifier: MIT

pragma solidity ^0.8.0;

import { ITimeLock } from '../interfaces/ITimeLock.sol';

/**
 * @title TimeLock
 * @dev A contract that enables function execution after a certain time has passed.
 * Implements the {ITimeLock} interface.
 */
contract TimeLock is ITimeLock {
    bytes32 internal constant PREFIX_TIME_LOCK = keccak256('time-lock');

    uint256 internal immutable _minimumTimeLockDelay;

    /**
     * @notice The constructor for the TimeLock.
     * @param minimumTimeDelay The minimum time delay (in secs) that must pass for the TimeLock to be executed
     */
    constructor(uint256 minimumTimeDelay) {
        _minimumTimeLockDelay = minimumTimeDelay;
    }

    /**
     * @notice Returns a minimum time delay at which the TimeLock may be scheduled.
     * @return uint Minimum scheduling delay time (in secs) from the current block timestamp
     */
    function minimumTimeLockDelay() external view override returns (uint256) {
        return _minimumTimeLockDelay;
    }

    /**
     * @notice Returns the timestamp after which the TimeLock may be executed.
     * @param hash The hash of the timelock
     * @return uint The timestamp after which the timelock with the given hash can be executed
     */
    function getTimeLock(bytes32 hash) external view override returns (uint256) {
        return _getTimeLockEta(hash);
    }

    /**
     * @notice Schedules a new timelock.
     * @dev The timestamp will be set to the current block timestamp + minimum time delay,
     * if the provided timestamp is less than that.
     * @param hash The hash of the new timelock
     * @param eta The proposed Unix timestamp (in secs) after which the new timelock can be executed
     * @return uint The Unix timestamp (in secs) after which the new timelock can be executed
     */
    function _scheduleTimeLock(bytes32 hash, uint256 eta) internal returns (uint256) {
        if (hash == 0) revert InvalidTimeLockHash();
        if (_getTimeLockEta(hash) != 0) revert TimeLockAlreadyScheduled();

        uint256 minimumEta = block.timestamp + _minimumTimeLockDelay;

        if (eta < minimumEta) eta = minimumEta;

        _setTimeLockEta(hash, eta);

        return eta;
    }

    /**
     * @notice Cancels an existing timelock by setting its eta to zero.
     * @param hash The hash of the timelock to cancel
     */
    function _cancelTimeLock(bytes32 hash) internal {
        if (hash == 0) revert InvalidTimeLockHash();

        _setTimeLockEta(hash, 0);
    }

    /**
     * @notice Finalizes an existing timelock and sets its eta back to zero.
     * @dev To finalize, the timelock must currently exist and the required time delay
     * must have passed.
     * @param hash The hash of the timelock to finalize
     */
    function _finalizeTimeLock(bytes32 hash) internal {
        uint256 eta = _getTimeLockEta(hash);

        if (hash == 0 || eta == 0) revert InvalidTimeLockHash();

        if (block.timestamp < eta) revert TimeLockNotReady();

        _setTimeLockEta(hash, 0);
    }

    /**
     * @dev Returns the timestamp after which the timelock with the given hash can be executed.
     */
    function _getTimeLockEta(bytes32 hash) internal view returns (uint256 eta) {
        bytes32 key = keccak256(abi.encodePacked(PREFIX_TIME_LOCK, hash));

        assembly {
            eta := sload(key)
        }
    }

    /**
     * @dev Sets a new timestamp for the timelock with the given hash.
     */
    function _setTimeLockEta(bytes32 hash, uint256 eta) private {
        bytes32 key = keccak256(abi.encodePacked(PREFIX_TIME_LOCK, hash));

        assembly {
            sstore(key, eta)
        }
    }
}
