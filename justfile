default:
	just --list

test:
	cargo test

check:
	pre-commit

# generate rust bindings for contracts
gen-bindings:
	forge bind --contracts ./contracts/src/ --crate-name contract-bindings --bindings-path contract-bindings --overwrite --force
	cargo fmt --all
	cargo sort -g -w

# Lint solidity files
sol-lint:
	forge fmt
	solhint --fix 'contracts/{script,src,test}/**/*.sol'

sol-test:
	forge test

# Deploy contracts to local blockchain for development and testing
dev-deploy url="http://localhost:8545" mnemonics="test test test test test test test test test test test junk" num_blocks_per_epoch="10" num_init_validators="5":
	MNEMONICS="{{mnemonics}}" forge script contracts/test/LightClientTest.s.sol:DeployLightClientTestScript \
	--sig "run(uint32 numBlocksPerEpoch, uint32 numInitValidators)" {{num_blocks_per_epoch}} {{num_init_validators}} \
	--fork-url {{url}} --broadcast
