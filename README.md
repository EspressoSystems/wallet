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

## Local testing

Download the wallet binary:

### Linux

    curl -Lo wallet https://github.com/EspressoSystems/wallet/releases/latest/download/wallet-x86-linux
    chmod +x wallet

### macOS

    arch=$(uname -m | sed 's/x86_64/x86/;s/arm64/aarch64/')
    curl -Lo wallet https://github.com/EspressoSystems/wallet/releases/latest/download/wallet-$arch-darwin
    chmod +x wallet

### Windows

    curl -Lo wallet.exe https://github.com/EspressoSystems/wallet/releases/latest/download/wallet-x86-win.exe

If you would like to manually download the binaries you can find them on our github [releases](https://github.com/EspressoSystems/wallet/tags) page.

### Run the sequencer and nitro deployment locally
Make sure you have `docker` installed and running.

Start the local sequencer and nitro

    scripts/run-sequencer

You may have to wait for about a minute until everything is up and running.

Run the wallet commands with the nitro config file, for example

    ./wallet -c config.toml.nitro balance

On windows replace `./wallet` with `wallet`.

Once the balance command runs successfully and returns a balance you can try
other commands.

    ./wallet --help

will show all available commands.

## Development

Dependencies can be installed with [nix](https://nixos.org/download/) by running
`nix develop` or by using [`direnv`](https://direnv.net/).

To use without nix install a rust toolchain and
[foundry](https://book.getfoundry.sh/getting-started/installation).

To run the tests, first build the wallet executable then run the tests:

    cargo build
    cargo test

To run the integration test with nitro docker also needs to be installed and
running.

## Contract deployment

To deploy locally, first start a dev node by running `anvil` in one terminal,
then deploy by running

    forge script contracts/script/DeploySimpleToken.s.sol --broadcast --rpc-url local

To deploy on sepolia set `MNEMONIC` to your sepolia URL and run

    env MNEMONIC="..." forge script contracts/script/DeploySimpleToken.s.sol --broadcast --rpc-url sepolia

Commit the new files in `contracts/broadcast` after doing a sepolia deployment
so downstream consumers can seet the contract address(es) from these files.
