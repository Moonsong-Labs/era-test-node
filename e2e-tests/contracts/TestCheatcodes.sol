// SPDX-License-Identifier: MIT

pragma solidity ^0.8.0;

contract TestCheatcodes {
  address constant CHEATCODE_ADDRESS = 0x7109709ECfa91a80626fF3989D68f67F5b1DD12D;

  function testDeal(address account, uint256 amount) external {
    uint balanceBefore = address(account).balance;
    (bool success, ) = CHEATCODE_ADDRESS.call(abi.encodeWithSignature("deal(address,uint256)", account, amount));
    uint balanceAfter = address(account).balance;
    require(balanceAfter == amount, "balance mismatch");
    require(balanceAfter != balanceBefore, "balance unchanged");
    require(success, "deal failed");
  }

  function testEtch(address target, bytes calldata code) external {
    (bool success, ) = CHEATCODE_ADDRESS.call(abi.encodeWithSignature("etch(address,bytes)", target, code));
    require(success, "etch failed");
    (success, ) = target.call(abi.encodeWithSignature("setGreeting(bytes)", bytes("hello world")));
    require(success, "setGreeting failed");
  }

  function setNonce(address account, uint256 nonce) external {
    (bool success, ) = CHEATCODE_ADDRESS.call(abi.encodeWithSignature("setNonce(address,uint64)", account, nonce));
    require(success, "setNonce failed");
  }
}
