use ethers::types::Address;
use serde::{Deserialize, Serialize};
use url::Url;

#[derive(Serialize, Deserialize)]
struct EthKey {
    verifying_key: String,
    address: String,
}

pub fn get_builder_address(url: Url) -> Address {
    let url = url.join("block_info/builderaddress").unwrap();
    let body = reqwest::blocking::get(url)
        .unwrap()
        .json::<EthKey>()
        .unwrap();

    body.address.parse::<Address>().unwrap()
}
