default:
    just --list

test *args:
    cargo test {{args}}

nitro-test *args:
	cargo test --release nitro::test {{args}}

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
