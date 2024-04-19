use ethers::types::Address;
use serde::{Deserialize, Serialize};
use url::Url;

#[derive(Serialize, Deserialize)]
struct EthKey {
    verifying_key: String,
    address: String,
}

pub async fn get_builder_address(url: Url) -> Address {
    let url = url.join("block_info/builderaddress").unwrap();
    let body = reqwest::get(url)
        .await
        .unwrap()
        .json::<EthKey>()
        .await
        .unwrap();

    body.address.parse::<Address>().unwrap()
}
