# Cove

This repo contains the backend verification[^1] code for Cove, a simple, reliable, open-source **co**ntract **ve**rification built for an L2 centric Ethereum ecosystem.

## Why?

The current state of smart contract verification has a lot of room for improvement:

- Verification often fails, with no useful feedback as to why.
  - There are ~200 total [Foundry](https://github.com/foundry-rs/foundry/issues?q=etherscan+verification) and [Hardhat](https://github.com/NomicFoundation/hardhat/issues?q=etherscan+verification) verification issues.
- Every Layer 2 chain has a different block explorer
  - Need to manually verify with each verification provider.
  - Verifying on every single chain doesn't scale for developers.
- Verified contracts are not linked to git commits.
  - Hard to verify the audited code is what’s actually deployed.
  - Hard to verify yourself if you don't trust the hosted verification.
- 1:1 mapping of verification providers to UIs
  - Anyone can spin up a novel frontend to interact with a smart contract, but not to view verified contracts

## The State of Cove

- [x] Verify contracts in forge projects with just a repo URL, commit hash, and contract address.
- [x] Verify contracts on all supported chains with a single API call.
- [x] Return decompiled bytecode, ABI, and Solidity for unverified contracts[^2].
- [ ] More robust verification for all contracts (i.e. smarter bytecode matching and fallbacks).
- [ ] Save verified contracts to a publicly available database.
- [ ] Support other development frameworks such as Hardhat and Ape.
- [ ] Support other languages such as Vyper and Huff.
- [ ] Publish the Cove backend as a crate for easy local verification.
- [ ] Multi-file verification orders files logically.
- [ ] Automatically verify on Etherscan and Sourcify after successful verification.
- [ ] Support traditional methods of verification (e.g. standard JSON input).
- [ ] Build a first-party UI to showcase the functionality of Cove.

[^1]: The frontend can be found in the [ScopeLift/cove-frontend](https://github.com/ScopeLift/cove-frontend) repo.
[^2]: Thanks to [heimdall-rs](https://github.com/Jon-Becker/heimdall-rs) by @Jon-Becker
