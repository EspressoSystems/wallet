use ethers::types::Address;
use url::Url;

pub async fn get_builder_address(url: String) -> Address {
    let url = Url::parse(&url).unwrap();
    let url = url.join("builderaddress").unwrap();
    let body = reqwest::get(url).await.unwrap().text().await.unwrap();

    println!("body = {body:?}");
    body.to_string().parse::<Address>().unwrap()
}
