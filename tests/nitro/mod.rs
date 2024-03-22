use std::{
    process::{Command, Stdio},
    sync::Arc,
    time::Duration,
};

use anyhow::Result;
use ethers::{prelude::*, signers::coins_bip39::English};

use crate::wait_for_condition;
use contracts::simple_token::SimpleToken;

#[ignore = "wip"]
#[async_std::test]
async fn test() -> Result<()> {
    // build the release first
    Command::new("cargo")
        .arg("build")
        .arg("--release")
        .spawn()?;

    let nitro_work_dir = "tests/nitro/nitro-testnode";
    Command::new("docker")
        .current_dir(nitro_work_dir)
        .arg("compose")
        .arg("down")
        .spawn()?;

    let _ = Command::new("./test-node.bash")
        .current_dir(nitro_work_dir)
        .arg("--init")
        .arg("--espresso")
        .arg("--latest-espresso-image")
        .stdout(Stdio::null())
        .spawn()?;

    let commitment_task_is_good = wait_for_condition(
        || async {
            let output = Command::new("curl")
                .arg("http://localhost:60000/api/hotshot_contract")
                .output();
            if let Err(e) = output {
                eprintln!("{}", e);
                false
            } else {
                let output = output.unwrap();
                !output.stdout.is_empty()
            }
        },
        Duration::from_secs(5),
        Duration::from_secs(90),
    )
    .await;
    assert!(commitment_task_is_good);

    let mnemonic = "indoor dish desk flag debris potato excuse depart ticket judge file exit";
    let index = 6_u32;
    let nitro_rpc = "http://127.0.0.1:8547";
    let provider = Provider::<Http>::try_from(nitro_rpc)?.interval(Duration::from_millis(10));
    let wallet = MnemonicBuilder::<English>::default()
        .phrase(mnemonic)
        .index(index)?
        .build()?
        .with_chain_id(412346_u64);
    let client = SignerMiddleware::new(provider, wallet);
    let addr = client.address();
    let _ = wait_for_condition(
        || async {
            match client.get_balance(addr, None).await {
                Ok(num) => {
                    println!("{:?}", num);
                    // wait for sufficient blocks
                    num > 0.into()
                }
                Err(e) => {
                    eprintln!("failed to get block number: {:?}", e);
                    false
                }
            }
        },
        Duration::from_secs(5),
        Duration::from_secs(300),
    )
    .await;
    let l2_is_good = wait_for_condition(
        || async {
            let output = client.get_block_number().await;
            match output {
                Ok(b) => b > 50.into(),
                Err(_) => false,
            }
        },
        Duration::from_secs(5),
        Duration::from_secs(90),
    )
    .await;
    assert!(l2_is_good);

    let wallet_dir = "target/nix/release";
    let balance_output = Command::new("wallet")
        .arg("balance")
        .env("MNEMONIC", mnemonic)
        .env("ROLLUP_RPC_URL", nitro_rpc)
        .env("ACCOUNT_INDEX", index.to_string())
        .current_dir(wallet_dir)
        .output()?;

    println!("{:?}", balance_output.stdout);

    let transfer_output = Command::new("wallet")
        .arg("transfer")
        .arg("--to")
        .arg(format!("{:x}", Address::random()))
        .arg("--amount")
        .arg("10")
        .env("MNEMONIC", mnemonic)
        .env("ROLLUP_RPC_URL", nitro_rpc)
        .env("ACCOUNT_INDEX", index.to_string())
        .output()?;

    println!("receipt: {:?}", String::from_utf8(transfer_output.stdout));
    println!(
        "error: {:?}",
        String::from_utf8(transfer_output.stderr.clone())
    );
    assert!(transfer_output.stderr.is_empty());

    let erc20 = SimpleToken::deploy(
        Arc::new(client),
        ("name".to_string(), "symbol".to_string(), U256::from(18)),
    )
    .unwrap()
    .send()
    .await?;

    let erc20_addr = format!("{:x}", erc20.address());
    let output = Command::new("wallet")
        .arg("mint_erc20")
        .arg("--contract-address")
        .arg(erc20_addr)
        .arg("--to")
        .arg(format!("{:x}", addr))
        .arg("--amount")
        .arg("1")
        .env("MNEMONIC", mnemonic)
        .env("ROLLUP_RPC_URL", nitro_rpc)
        .env("ACCOUNT_INDEX", index.to_string())
        .output()?;

    println!("{:?}", output);
    assert!(output.status.success());
    Ok(())
}

#[test]
fn address_test() {
    use std::str::FromStr;
    let addr = format!("{:x}", Address::random());
    println!("{:?}", addr);
    Address::from_str(&addr).unwrap();
}
