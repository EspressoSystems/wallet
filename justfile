default:
    just --list

test:
    cargo build # build the wallet executable
    cargo test

nitro-test:
	cargo test -p wallet nitro::test

check:
    pre-commit

# generate rust bindings for contracts
gen-bindings:
    forge bind --select '^SimpleToken$' --contracts ./contracts/src --crate-name contract-bindings --bindings-path contract-bindings --overwrite --force
    cargo fmt --all
    cargo sort -g -w

# Lint solidity files
sol-lint:
    solhint --fix 'contracts/{src,script,test}/**/*.sol'
