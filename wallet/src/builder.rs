use ethers::types::Address;
use std::{thread::sleep, time::Duration};
use url::Url;

pub fn get_builder_address(url: Url) -> Address {
    let url = url.join("block_info/builderaddress").unwrap();
    for _ in 0..5 {
        if let Ok(body) = reqwest::blocking::get(url.clone()) {
            return body.json::<Address>().unwrap();
        } else {
            sleep(Duration::from_millis(400))
        }
    }
    panic!("Error: Failed to retrieve address from builder!");
}
