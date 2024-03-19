default:
	just --list

test:
	cargo test

check:
	pre-commit

# generate rust bindings for contracts
gen-bindings:
	forge bind --contracts ./sol/ --module --bindings-path src/contracts --overwrite --force
	cargo fmt --all
	cargo sort -g -w

# Lint solidity files
sol-lint:
	forge fmt
	solhint --fix 'contracts/{script,src,test}/**/*.sol'
