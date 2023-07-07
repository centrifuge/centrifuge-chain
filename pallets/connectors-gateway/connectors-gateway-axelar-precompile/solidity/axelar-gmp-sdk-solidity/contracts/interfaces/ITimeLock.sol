// SPDX-License-Identifier: MIT

pragma solidity ^0.8.0;

/**
 * @title ITimeLock
 * @dev Interface for a TimeLock that enables function execution after a certain time has passed.
 */
interface ITimeLock {
    error InvalidTimeLockHash();
    error TimeLockAlreadyScheduled();
    error TimeLockNotReady();

    /**
     * @notice Returns a minimum time delay at which the TimeLock may be scheduled.
     * @return uint Minimum scheduling delay time (in secs) from the current block timestamp
     */
    function minimumTimeLockDelay() external view returns (uint256);

    /**
     * @notice Returns the timestamp after which the TimeLock may be executed.
     * @param hash The hash of the timelock
     * @return uint The timestamp after which the timelock with the given hash can be executed
     */
    function getTimeLock(bytes32 hash) external view returns (uint256);
}
