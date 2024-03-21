default:
    just --list

test:
    cargo build # build the wallet executable
    cargo test

check:
    pre-commit

# generate rust bindings for contracts
gen-bindings:
	forge bind --contracts ./sol/ --crate-name contracts --bindings-path contracts --overwrite --force
	cargo fmt --all
	cargo sort -g -w

# Lint solidity files
sol-lint:
    solhint --fix 'sol/**/*.sol'
