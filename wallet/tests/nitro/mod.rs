use std::process::Output;
use std::sync::Arc;
use std::{
    process::{Command, Stdio},
    time::Duration,
};

use anyhow::Result;
use escargot::CargoBuild;
use ethers::{prelude::*, signers::coins_bip39::English};

use crate::{assert_output_is_receipt, wait_for_condition};
use contract_bindings::simple_token::SimpleToken;

const NITRO_WORK_DIR: &str = "../tests/nitro/nitro-testnode";

#[derive(Clone, Debug, Copy)]
enum OnError {
    Abort,
    Continue,
}

fn process_command_output(on_error: OnError, output: &Output) {
    if !output.status.success() {
        match on_error {
            OnError::Abort => panic!("Command failed, aborting: {:?}", output.stderr),
            OnError::Continue => {
                println!("Command failed, trying to continue: {:?}", output.stderr)
            }
        }
    }
}

fn stop_and_remove_nitro(on_error: OnError) {
    println!("Stopping nitro and removing containers");
    let output = Command::new("docker")
        .current_dir(NITRO_WORK_DIR)
        .arg("compose")
        .arg("down")
        .output()
        .unwrap();
    process_command_output(on_error, &output);

    let output = Command::new("docker")
        .arg("ps")
        .arg("-aq")
        .arg("--filter")
        .arg("label=com.docker.compose.project=nitro-testnode")
        .output()
        .unwrap();
    process_command_output(on_error, &output);

    let output_str = std::str::from_utf8(&output.stdout).unwrap().trim();
    if !output_str.is_empty() {
        let containers = output_str.split('\n').collect::<Vec<_>>();

        println!("Stopping containers {:?}", containers);
        Command::new("docker")
            .arg("stop")
            .args(&containers)
            .output()
            .unwrap();
        process_command_output(on_error, &output);

        println!("Removing containers {:?}", containers);
        Command::new("docker")
            .arg("rm")
            .args(&containers)
            .output()
            .unwrap();
        process_command_output(on_error, &output);
    }

    let output = Command::new("docker")
        .arg("volume")
        .arg("ls")
        .arg("-q")
        .arg("--filter")
        .arg("label=com.docker.compose.project=nitro-testnode")
        .output()
        .unwrap();
    process_command_output(on_error, &output);

    let output_str = std::str::from_utf8(&output.stdout).unwrap().trim();
    if !output_str.is_empty() {
        let volumes = output_str.split('\n').collect::<Vec<_>>();
        println!("Removing volumes {:?}", volumes);
        let output = Command::new("docker")
            .arg("volume")
            .arg("rm")
            .args(volumes)
            .output()
            .unwrap();
        process_command_output(on_error, &output);
    }
}

struct Cleanup;
impl Drop for Cleanup {
    fn drop(&mut self) {
        // Try our best to clean up at the end of the test
        stop_and_remove_nitro(OnError::Continue);
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
    // Abort if we can't create a clean state before the test.
    stop_and_remove_nitro(OnError::Abort);
    let _teardown = Cleanup;

    // Sanity test to assert that we can locate the binary.
    let output = run_wallet().arg("--help").output()?;
    dbg!(&output);
    assert!(output.status.success());

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
    let provider = Provider::<Http>::try_from(nitro_rpc)?.interval(Duration::from_secs(5));
    let wallet = MnemonicBuilder::<English>::default()
        .phrase(mnemonic)
        .index(index)?
        .build()?
        .with_chain_id(412346_u64);
    let client = SignerMiddleware::new(provider, wallet);
    let addr = client.address();

    // Check the funding
    let funded = wait_for_condition(
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
    assert!(funded);

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

    dotenv::from_path(format!("{}/.env", NITRO_WORK_DIR)).unwrap();
    let builder_url = format!(
        "http://localhost:{}",
        dotenv::var("ESPRESSO_BUILDER_SERVER_PORT").unwrap()
    );
    let commitment_task_url = format!(
        "http://localhost:{}/api/hotshot_contract",
        dotenv::var("ESPRESSO_COMMITMENT_TASK_PORT").unwrap(),
    );
    let commitment_task_is_good = wait_for_condition(
        || async {
            match reqwest::get(&commitment_task_url).await {
                Ok(body) => !body.text().await.unwrap().is_empty(),
                Err(e) => {
                    eprintln!("{}", e);
                    false
                }
            }
        },
        Duration::from_secs(5),
        Duration::from_secs(300),
    )
    .await;
    assert!(commitment_task_is_good);

    println!("Checking balance");
    let balance_output = run_wallet()
        .arg("balance")
        .env("MNEMONIC", mnemonic)
        .env("ROLLUP_RPC_URL", nitro_rpc)
        .env("ACCOUNT_INDEX", index.to_string())
        .output()?;
    assert!(balance_output.status.success());

    println!("Doing a transfer");
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

    println!("Doing a transfer with invalid builder address");
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

    println!("Doing a transfer with valid builder address");
    let dummy_address = format!("0x{:x}", Address::from_slice(&[2u8; 20]));

    let valid_builder_address =
        dotenv::var("ESPRESSO_SEQUENCER_PREFUNDED_BUILDER_ACCOUNTS").unwrap();
    let transfer_with_valid_builder = run_wallet()
        .arg("transfer")
        .arg("--to")
        .arg(dummy_address.clone())
        .arg("--amount")
        .arg("10")
        .arg("--guaranteed-by-builder")
        .env("MNEMONIC", mnemonic)
        .env("ROLLUP_RPC_URL", nitro_rpc)
        .env("ACCOUNT_INDEX", index.to_string())
        .env("BUILDER_ADDRESS", &valid_builder_address)
        .output()?;
    assert!(transfer_with_valid_builder.status.success());

    println!("Transfer with Builder URL");
    let transfer_with_builder_url = run_wallet()
        .arg("transfer")
        .arg("--to")
        .arg(dummy_address)
        .arg("--amount")
        .arg("10")
        .arg("--guaranteed-by-builder")
        .env("MNEMONIC", mnemonic)
        .env("ROLLUP_RPC_URL", nitro_rpc)
        .env("ACCOUNT_INDEX", index.to_string())
        .env("BUILDER_URL", builder_url)
        .output()?;
    assert!(transfer_with_builder_url.status.success());

    println!("Deploying ERC20 token");
    let contract = SimpleToken::deploy(
        Arc::new(client),
        ("name".to_string(), "symbol".to_string(), U256::from(18)),
    )?
    .send()
    .await?;

    let erc20_addr = &format!("{:x}", contract.address());

    println!("Minting ERC20 tokens");
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

    println!("Minting ERC20 tokens with selected builder");
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
        .env("BUILDER_ADDRESS", &valid_builder_address)
        .output()?;

    assert!(output.status.success());

    println!("Checking ERC20 balance");
    let output = run_wallet()
        .arg("balance-erc20")
        .arg("--contract-address")
        .arg(erc20_addr)
        .env("MNEMONIC", mnemonic)
        .env("ROLLUP_RPC_URL", nitro_rpc)
        .env("ACCOUNT_INDEX", index.to_string())
        .output()?;

    assert!(output.status.success());

    println!("Transferring ERC20 tokens");
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

    println!("Transferring ERC20 tokens with selected builder");
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
