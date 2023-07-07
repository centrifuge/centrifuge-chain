// SPDX-License-Identifier: MIT

pragma solidity ^0.8.0;

import { IERC20 } from '../../interfaces/IERC20.sol';

contract DestinationChainTokenSwapper {
    error WrongTokenPair();

    address public tokenA;
    address public tokenB;

    constructor(address tokenA_, address tokenB_) {
        tokenA = tokenA_;
        tokenB = tokenB_;
    }

    function swap(
        address tokenAddress,
        address toTokenAddress,
        uint256 amount,
        address recipient
    ) external returns (uint256 convertedAmount) {
        IERC20(tokenAddress).transferFrom(msg.sender, address(this), amount);

        if (tokenAddress == tokenA) {
            if (toTokenAddress != tokenB) revert WrongTokenPair();

            convertedAmount = amount * 2;
        } else {
            if (tokenAddress != tokenB || toTokenAddress != tokenA) revert WrongTokenPair();

            convertedAmount = amount / 2;
        }

        IERC20(toTokenAddress).transfer(recipient, convertedAmount);
    }
}
