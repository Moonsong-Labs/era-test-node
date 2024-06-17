// SPDX-License-Identifier: GPL-3.0

pragma solidity >=0.8.2 <0.9.0;

contract Return5Inner {
  function value() public pure returns (uint8) {
    return 15;
  }
}

contract Return5 {
  Return5Inner inner;

  constructor() {
    inner = new Return5Inner();
  }

  function value() public view returns (uint8) {
    return inner.value();
  }
}
