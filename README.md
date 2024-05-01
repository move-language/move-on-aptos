
[![License](https://img.shields.io/badge/license-Apache-green.svg)](LICENSE)

![Move logo](assets/color/PNG/MoveOnAptos.png)

# The Move Language 

Move is a programming language for writing safe smart contracts originally developed at Facebook to power the Diem blockchain. Move is designed to be a platform-agnostic language to enable common libraries, tooling, and developer communities across diverse blockchains with vastly different data and execution models.

This repository is a mirror of *Move on Aptos*, a natural evolution of the Move language how it was originally designed. The repository contains the generic part of the Move language and implementation as it is used on the [Aptos Network](https://aptosfoundation.org/) and on other networks. Major components of the implementation include 
the Move virtual machine, bytecode verifier, compiler, prover, and package manager.

For an introduction into the Move language, please check out the following resources:

- The [Move Landing Page](https://aptos.dev/move/move-on-aptos) in the Aptos developer documentation
- TODO: more resources

> *NOTE* If you have an issue with Move on Aptos while working with Aptos, please open it [here](https://github.com/aptos-labs/aptos-core/issues/new/choose) and not in this repo.

# Consuming and Contributing to Move

The repository mirrors the content of the `aptos-core` repo, subtree [`third_party/move`](https://github.com/aptos-labs/aptos-core/tree/main/third_party/move). By consuming this repo, one can avoid cloning the large aptos-core repo. The repo is updated on a monthly basis with the newest changes from aptos-core.

> TODO: we plan to automate the update with a nightly schedule

For contributions, we ask for now to submit those to the [aptos-core](https://github.com/aptos-labs/aptos-core/tree/main/third_party/move) repo. 

> TODO: come up with a workflow to allow contributions downstream and upstream


## License

Move is licensed as [Apache 2.0](https://github.com/move-language/move/blob/main/LICENSE).
