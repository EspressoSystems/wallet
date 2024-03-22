// SPDX-License-Identifier: UNLICENSED
pragma solidity ^0.8.0;

import "forge-std/Script.sol";
import { SimpleToken } from "../src/SimpleToken.sol";

contract DeploySimpleTokenScript is Script {
    function run() external {
        string memory seedPhrase = vm.envString("MNEMONIC");
        uint256 privateKey = vm.deriveKey(seedPhrase, 0);
        vm.startBroadcast(privateKey);

        new SimpleToken( /*name */ "Bean", /* symbol */ "BEAN", /* decimals */ 18);

        vm.stopBroadcast();
    }
}
