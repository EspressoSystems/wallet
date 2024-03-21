use std::{io::Write, process::Command, sync::Arc, time::Duration};

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
    let nitro_work_dir = "tests/nitro/testnode";
    Command::new("docker")
        .current_dir(nitro_work_dir)
        .arg("compose")
        .arg("down")
        .spawn()?;
    let mut testnode = Command::new("./test-node.bash")
        .current_dir(nitro_work_dir)
        .arg("--init")
        .arg("--espresso")
        .arg("--latest-espresso-image")
        .spawn()?;

    if let Some(stdin) = testnode.stdin.as_mut() {
        stdin.write_all(b"y\n")?;
    }

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
        Duration::from_secs(60),
    )
    .await;
    assert!(commitment_task_is_good);

    let mnemonic = "indoor dish desk flag debris potato excuse depart ticket judge file exit";
    let index = 0_u32;
    let nitro_rpc = "http://127.0.0.1:8547";
    let provider = Provider::<Http>::try_from(nitro_rpc)?.interval(Duration::from_millis(10));
    let wallet = MnemonicBuilder::<English>::default()
        .phrase(mnemonic)
        .index(index)?
        .build()?;
    let client = SignerMiddleware::new(provider, wallet);
    let _ = wait_for_condition(
        || async {
            if let Ok(num) = client.get_block_number().await {
                num > 10.into()
            } else {
                false
            }
        },
        Duration::from_secs(5),
        Duration::from_secs(300),
    )
    .await;

    SimpleToken::deploy(
        Arc::new(client),
        ("name".to_string(), "symbol".to_string(), U256::from(18)),
    )
    .unwrap()
    .send()
    .await?;

    let wallet_dir = "target/nix/release";
    let output = Command::new("wallet")
        .arg("balance")
        .env("MNEMONIC", mnemonic)
        .env("ROLLUP_RPC_URL", nitro_rpc)
        .env("ACCOUNT_INDEX", index.to_string())
        .current_dir(wallet_dir)
        .output()?;

    println!("{:?}", output.stdout);
    Ok(())
}
