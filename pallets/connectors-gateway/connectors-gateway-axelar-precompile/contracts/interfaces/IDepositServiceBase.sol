// SPDX-License-Identifier: MIT

pragma solidity ^0.8.0;

interface IDepositServiceBase {
    error InvalidAddress();
    error InvalidSymbol();
    error InvalidAmount();
    error NothingDeposited();
    error WrapFailed();
    error UnwrapFailed();
    error TokenApproveFailed();
    error NotRefundIssuer();
    error WrappedTokenNotSupported();

    function gateway() external returns (address);

    function wrappedSymbol() external returns (string memory);

    function wrappedToken() external returns (address);
}
