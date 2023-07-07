// SPDX-License-Identifier: MIT

pragma solidity 0.8.9;

contract DepositHandler {
    error IsLocked();
    error NotContract();

    uint256 internal constant IS_NOT_LOCKED = uint256(1);
    uint256 internal constant IS_LOCKED = uint256(2);

    uint256 internal _lockedStatus = IS_NOT_LOCKED;

    modifier noReenter() {
        if (_lockedStatus == IS_LOCKED) revert IsLocked();

        _lockedStatus = IS_LOCKED;
        _;
        _lockedStatus = IS_NOT_LOCKED;
    }

    function execute(address callee, bytes calldata data) external noReenter returns (bool success, bytes memory returnData) {
        if (callee.code.length == 0) revert NotContract();
        (success, returnData) = callee.call(data);
    }

    // NOTE: The gateway should always destroy the `DepositHandler` in the same runtime context that deploys it.
    function destroy(address etherDestination) external noReenter {
        selfdestruct(payable(etherDestination));
    }
}
