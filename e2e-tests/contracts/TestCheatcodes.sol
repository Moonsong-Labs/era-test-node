// SPDX-License-Identifier: MIT

pragma solidity ^0.8.0;

contract TestCheatcodes {
  address constant CHEATCODE_ADDRESS = 0x7109709ECfa91a80626fF3989D68f67F5b1DD12D;

  function testAddr(uint256 privateKey, address addr) external {
    (bool success, bytes memory data) = CHEATCODE_ADDRESS.call(abi.encodeWithSignature("addr(uint256)", privateKey));
    require(success, "addr failed");
    address recovered = abi.decode(data, (address));
    require(recovered == addr, "address mismatch");
  }

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
    (success, ) = target.call(abi.encodeWithSignature("setGreeting(string)", "hello world"));
    require(success, "setGreeting failed");
  }

  function testSetNonce(address account, uint64 nonce) external {
    (bool success, bytes memory data) = CHEATCODE_ADDRESS.call(
      abi.encodeWithSignature("setNonce(address,uint64)", account, nonce)
    );
    require(success, "setNonce failed");
    (success, data) = CHEATCODE_ADDRESS.call(abi.encodeWithSignature("getNonce(address)", account));
    require(success, "getNonce failed");
    uint64 finalNonce = abi.decode(data, (uint64));
    require(finalNonce == nonce, "nonce mismatch");
  }

  function testRoll(uint256 blockNumber) external {
    uint256 initialBlockNumber = block.number;
    require(blockNumber != initialBlockNumber, "block number must be different than current block number");

    (bool success, ) = CHEATCODE_ADDRESS.call(abi.encodeWithSignature("roll(uint256)", blockNumber));
    require(success, "roll failed");

    uint256 finalBlockNumber = block.number;
    require(finalBlockNumber == blockNumber, "block number was not changed");
  }

  function testWarp(uint256 timestamp) external {
    uint256 initialTimestamp = block.timestamp;
    require(timestamp != initialTimestamp, "timestamp must be different than current block timestamp");

    (bool success, ) = CHEATCODE_ADDRESS.call(abi.encodeWithSignature("warp(uint256)", timestamp));
    require(success, "warp failed");

    uint256 finalTimestamp = block.timestamp;
    require(finalTimestamp == timestamp, "timestamp was not changed");
  }
}
