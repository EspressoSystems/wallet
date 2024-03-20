use ethers::types::Address;

pub fn get_builder_address() -> Address {
    Address::random()
}
