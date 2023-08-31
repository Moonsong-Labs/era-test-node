import { Deployer } from "@matterlabs/hardhat-zksync-deploy";
import { expect } from "chai";
import * as hre from "hardhat";
import { Contract, Wallet } from "zksync-web3";
import { RichAccounts } from "../helpers/constants";
import { deployContract } from "../helpers/utils";

describe("Sample Test Suite", function () {
  let greeter: Contract;

  before("deploying a fresh contract", async function () {
    const wallet = new Wallet(RichAccounts[0].PrivateKey);
    const deployer = new Deployer(hre, wallet);
    greeter = await deployContract(deployer, "Greeter", ["Hi"]);
  });

  it("Should be deployed with initial greeting", async function () {
    expect(await greeter.greet()).to.eq("Hi");
  });

  it("Should return the new greeting once it's changed", async function () {
    const setGreetingTx = await greeter.setGreeting("Hola, mundo!");
    // wait until the transaction is mined
    await setGreetingTx.wait();
    expect(await greeter.greet()).to.equal("Hola, mundo!");
  });

  it("Should be able to advance blocks by 2", async function () {
    const blockNumberBefore = await hre.ethers.provider.getBlockNumber();
    await hre.ethers.provider.send("evm_mine", []);
    await hre.ethers.provider.send("evm_mine", []);
    const blockNumberAfter = await hre.ethers.provider.getBlockNumber();
    expect(blockNumberAfter).to.equal(blockNumberBefore + 2);
  });
});
