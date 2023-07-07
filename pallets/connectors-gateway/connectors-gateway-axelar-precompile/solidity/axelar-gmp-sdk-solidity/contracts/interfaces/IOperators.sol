// SPDX-License-Identifier: MIT

pragma solidity ^0.8.0;

/**
 * @title IOperators Interface
 * @notice Interface for an access control mechanism where operators have exclusive
 * permissions to execute functions.
 */
interface IOperators {
    error NotOperator();
    error InvalidOperator();
    error OperatorAlreadyAdded();
    error NotAnOperator();
    error ExecutionFailed();

    event OperatorAdded(address indexed operator);
    event OperatorRemoved(address indexed operator);

    /**
     * @notice Check if an account is an operator.
     * @param account Address of the account to check
     * @return bool True if the account is an operator, false otherwise
     */
    function isOperator(address account) external view returns (bool);

    /**
     * @notice Adds an operator.
     * @param operator The address to add as an operator
     */
    function addOperator(address operator) external;

    /**
     * @notice Removes an operator.
     * @param operator The address of the operator to remove
     */
    function removeOperator(address operator) external;

    /**
     * @notice Executes an external contract call.
     * @dev Execution logic is left up to the implementation.
     * @param target The contract to call
     * @param callData The data to call the target contract with
     * @param nativeValue The amount of native asset to send with the call
     * @return bytes The data returned from the contract call
     */
    function execute(
        address target,
        bytes calldata callData,
        uint256 nativeValue
    ) external payable returns (bytes memory);
}
