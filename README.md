# Espresso Wallet

A command line interface wallet to interact with Espresso’s Cappuccino Testnet
deployment. The wallet can be use to send send transactions and queries to the
rollups running on the Cappuccino deployment. It also showcases a new feature
where a user can build a transaction that can only be included in blocks by a
builder of their choice, so-called "builder-guaranteed transactions".

The wallet currently supports sending the rollups native currencies and
creating, minting and transferring ERC20 tokens as well as checking the
corresponding balances.

To fetch the source code

    git clone --recursive https://github.com/EspressoSystems/wallet

## Dependencies

Dependencies can be install with [nix](https://nixos.org/download/) by running
`nix develop` or by using [`direnv`](https://direnv.net/).

To use without nix install a rust toolchain and
[foundry](https://book.getfoundry.sh/getting-started/installation).

## Development

To run the tests, first build the wallet executable then run the tests:

    cargo build
    cargo test

## Contract deployment

To deploy locally, first start a dev node by running `anvil` in one terminal,
then deploy by running

    forge script contracts/script/DeploySimpleToken.s.sol --broadcast --rpc-url local

To deploy on sepolia set `MNEMONIC` to your sepolia URL and run

    env MNEMONIC="..." forge script contracts/script/DeploySimpleToken.s.sol --broadcast --rpc-url sepolia

Commit the new files in `contracts/broadcast` after doing a sepolia deployment
so downstream consumers can seet the contract address(es) from these files.
