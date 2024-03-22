default:
    just --list

test:
    cargo build # build the wallet executable
    cargo test

check:
    pre-commit

# generate rust bindings for contracts
gen-bindings:
    forge bind --contracts ./contracts/src --crate-name contract-bindings --bindings-path contract-bindings --overwrite --force
    cargo fmt --all
    cargo sort -g -w

# Lint solidity files
sol-lint:
    solhint --fix 'contracts/**/*.sol'
