// SPDX-License-Identifier: MIT

pragma solidity ^0.8.0;

import { IERC721 } from './IERC721.sol';

interface IERC721MintableBurnable is IERC721 {
    function mint(address to, uint256 tokenId) external;

    function burn(uint256 tokenId) external;
}
