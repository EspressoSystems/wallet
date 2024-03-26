use std::sync::Arc;
use std::{
    process::{Command, Stdio},
    time::Duration,
};

use anyhow::Result;
use async_std::task::sleep;
use escargot::CargoBuild;
use ethers::{prelude::*, signers::coins_bip39::English};

use crate::{assert_output_is_receipt, wait_for_condition};
use contract_bindings::simple_token::SimpleToken;

const NITRO_WORK_DIR: &str = "../tests/nitro/nitro-testnode";

fn stop_and_remove_nitro() {
    println!("Stopping nitro and removing containers");
    let output = Command::new("docker")
        .current_dir(NITRO_WORK_DIR)
        .arg("compose")
        .arg("down")
        .output()
        .unwrap();
    assert!(output.status.success());

    let output = Command::new("docker")
        .arg("ps")
        .arg("-aq")
        .arg("--filter")
        .arg("label=com.docker.compose.project=nitro-testnode")
        .output()
        .unwrap();
    assert!(output.status.success());

    let output_str = std::str::from_utf8(&output.stdout).unwrap().trim();
    if !output_str.is_empty() {
        let containers = output_str.split("\n").collect::<Vec<_>>();
        println!("Removing containers {:?}", containers);
        Command::new("docker")
            .arg("rm")
            .args(containers)
            .output()
            .unwrap();
        assert!(output.status.success());
    }

    let output = Command::new("docker")
        .arg("volume")
        .arg("ls")
        .arg("-q")
        .arg("--filter")
        .arg("label=com.docker.compose.project=nitro-testnode")
        .output()
        .unwrap();
    assert!(output.status.success());

    let output_str = std::str::from_utf8(&output.stdout).unwrap().trim();
    if !output_str.is_empty() {
        let volumes = output_str.split("\n").collect::<Vec<_>>();
        println!("Removing volumes {:?}", volumes);
        let output = Command::new("docker")
            .arg("volume")
            .arg("rm")
            .args(volumes)
            .output()
            .unwrap();
        assert!(output.status.success());
    }
}

struct Cleanup;
impl Drop for Cleanup {
    fn drop(&mut self) {
        stop_and_remove_nitro();
    }
}

fn run_wallet() -> Command {
    CargoBuild::new()
        .bin("wallet")
        .current_release()
        .current_target()
        .run()
        .unwrap()
        .command()
}

#[async_std::test]
async fn test() -> Result<()> {
    stop_and_remove_nitro();
    let _teardown = Cleanup;

    // Sanity test to assert that we can locate the binary.
    let output = run_wallet().arg("--help").output()?;
    dbg!(&output);
    assert!(output.status.success());

    Command::new("docker")
        .current_dir(NITRO_WORK_DIR)
        .arg("compose")
        .arg("down")
        .output()?;

    let _ = Command::new("./test-node.bash")
        .current_dir(NITRO_WORK_DIR)
        .arg("--init")
        .arg("--espresso")
        .arg("--latest-espresso-image")
        .stdout(Stdio::null())
        .spawn()?;

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

    // Check the funding
    let _ = wait_for_condition(
        || async {
            println!("checking if nitro RPC is ready and client is funded");
            match client.get_balance(addr, None).await {
                Ok(num) => {
                    println!("current balance: {num}");
                    num > 0.into()
                }
                Err(e) => {
                    println!("failed to query balance: {e}");
                    false
                }
            }
        },
        Duration::from_secs(5),
        Duration::from_secs(300),
    )
    .await;

    // Wait for the testnode running completely
    let min_block_num = 50.into();
    let l2_is_good = wait_for_condition(
        || async {
            println!("waiting for nitro block number > {min_block_num}");
            let output = client.get_block_number().await;
            match output {
                Ok(b) => {
                    println!("block number: {b}");
                    b > min_block_num
                }
                Err(_) => {
                    println!("failed to get block number");
                    false
                }
            }
        },
        Duration::from_secs(5),
        Duration::from_secs(250),
    )
    .await;
    assert!(l2_is_good);

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
        Duration::from_secs(300),
    )
    .await;
    assert!(commitment_task_is_good);

    let balance_output = run_wallet()
        .arg("balance")
        .env("MNEMONIC", mnemonic)
        .env("ROLLUP_RPC_URL", nitro_rpc)
        .env("ACCOUNT_INDEX", index.to_string())
        .output()?;
    assert!(balance_output.status.success());

    let transfer_output = run_wallet()
        .arg("transfer")
        .arg("--to")
        .arg(format!("0x{:x}", Address::random()))
        .arg("--amount")
        .arg("10")
        .env("MNEMONIC", mnemonic)
        .env("ROLLUP_RPC_URL", nitro_rpc)
        .env("ACCOUNT_INDEX", index.to_string())
        .output()?;

    assert_output_is_receipt(transfer_output);
    let dummy_address = format!("0x{:x}", Address::from_slice(&[1u8; 20]));
    let transfer_with_invalid_builder = run_wallet()
        .arg("transfer")
        .arg("--to")
        .arg(dummy_address)
        .arg("--amount")
        .arg("10")
        .arg("--guaranteed-by-builder")
        .env("MNEMONIC", mnemonic)
        .env("ROLLUP_RPC_URL", nitro_rpc)
        .env("ACCOUNT_INDEX", index.to_string())
        .env("BUILDER_ADDRESS", format!("0x{:x}", Address::zero()))
        .output()?;
    assert!(!transfer_with_invalid_builder.status.success());

    let dummy_address = format!("0x{:x}", Address::from_slice(&[2u8; 20]));

    let valid_builder_address = "0x23618e81e3f5cdf7f54c3d65f7fbc0abf5b21e8f";
    let transfer_with_valid_builder = run_wallet()
        .arg("transfer")
        .arg("--to")
        .arg(dummy_address)
        .arg("--amount")
        .arg("10")
        .arg("--guaranteed-by-builder")
        .env("MNEMONIC", mnemonic)
        .env("ROLLUP_RPC_URL", nitro_rpc)
        .env("ACCOUNT_INDEX", index.to_string())
        .env("BUILDER_ADDRESS", valid_builder_address)
        .output()?;

    assert!(transfer_with_valid_builder.status.success());

    let _ = SimpleToken::deploy(
        Arc::new(client),
        ("name".to_string(), "symbol".to_string(), U256::from(18)),
    )
    .unwrap()
    .send()
    .await;
    // cannot `unwrap()` here

    sleep(Duration::from_secs(10)).await;

    let erc20_addr = "0xB7Fc0E52ec06F125F3afebA199248c79F71c2e3a";

    let output = run_wallet()
        .arg("mint-erc20")
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

    assert!(output.status.success());

    let output = run_wallet()
        .arg("mint-erc20")
        .arg("--contract-address")
        .arg(erc20_addr)
        .arg("--to")
        .arg(format!("{:x}", addr))
        .arg("--amount")
        .arg("1")
        .arg("--guaranteed-by-builder")
        .env("MNEMONIC", mnemonic)
        .env("ROLLUP_RPC_URL", nitro_rpc)
        .env("ACCOUNT_INDEX", index.to_string())
        .env("BUILDER_ADDRESS", valid_builder_address)
        .output()?;

    assert!(output.status.success());

    let output = run_wallet()
        .arg("balance-erc20")
        .arg("--contract-address")
        .arg(erc20_addr)
        .env("MNEMONIC", mnemonic)
        .env("ROLLUP_RPC_URL", nitro_rpc)
        .env("ACCOUNT_INDEX", index.to_string())
        .output()?;

    assert!(output.status.success());

    let output = run_wallet()
        .arg("transfer-erc20")
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

    assert!(output.status.success());

    let output = run_wallet()
        .arg("transfer-erc20")
        .arg("--contract-address")
        .arg(erc20_addr)
        .arg("--to")
        .arg(format!("{:x}", addr))
        .arg("--amount")
        .arg("1")
        .arg("--guaranteed-by-builder")
        .env("MNEMONIC", mnemonic)
        .env("ROLLUP_RPC_URL", nitro_rpc)
        .env("ACCOUNT_INDEX", index.to_string())
        .env("BUILDER_ADDRESS", valid_builder_address)
        .output()?;

    assert!(output.status.success());
    Ok(())
}
