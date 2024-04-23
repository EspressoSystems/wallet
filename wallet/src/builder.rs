use std::{thread::sleep, time::Duration};

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
    for _ in 0..5 {
        if let Ok(body) = reqwest::blocking::get(url.clone()) {
            return body
                .json::<EthKey>()
                .unwrap()
                .address
                .parse::<Address>()
                .unwrap();
        } else {
            sleep(Duration::from_millis(400))
        }
    }
    panic!("Error: Failed to retrieve address from builder!");
}
