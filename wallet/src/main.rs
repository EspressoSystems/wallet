mod builder;
mod wallet;

use builder::get_builder_address;
use clap::{Parser, Subcommand};
use ethers::types::{Address, U256};
use std::str::FromStr;
use wallet::EspressoWallet;

#[derive(Parser, Debug)]
pub struct Cli {
    #[clap(long, env = "MNEMONIC")]
    mnemonic: String,

    #[clap(long, env = "ROLLUP_RPC_URL")]
    rollup_rpc_url: String,

    #[clap(long, env = "BUILDER_URL", default_value = "")]
    builder_url: String,

    #[clap(long, env = "ACCOUNT_INDEX", default_value = "0")]
    account_index: u32,

    #[clap(subcommand)]
    commands: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    Transfer {
        /// hex string of the target address
        #[clap(long)]
        to: String,

        #[clap(long)]
        amount: u64,

        #[clap(long, default_value_t = false)]
        guaranteed_by_builder: bool,
    },
    TransferErc20 {
        #[clap(long)]
        contract_address: String,

        #[clap(long)]
        amount: u64,

        /// hex string of the target address
        #[clap(long)]
        to: String,

        #[clap(long, default_value_t = false)]
        guaranteed_by_builder: bool,
    },
    Balance,
    BalanceErc20 {
        #[clap(long)]
        contract_address: String,
    },
    MintErc20 {
        #[clap(long)]
        contract_address: String,

        #[clap(long)]
        amount: u64,

        /// hex string of the target address
        #[clap(long)]
        to: String,

        #[clap(long, default_value_t = false)]
        guaranteed_by_builder: bool,
    },
}

#[async_std::main]
async fn main() {
    let cli = Cli::parse();
    let wallet = EspressoWallet::new(cli.mnemonic, cli.account_index, cli.rollup_rpc_url);
    if let Err(e) = wallet {
        eprintln!("failed to create a wallet: {}", e);
        return;
    }
    let wallet = wallet.unwrap();

    match &cli.commands {
        Commands::Transfer {
            to,
            amount,
            guaranteed_by_builder,
        } => {
            let builder_addr = if *guaranteed_by_builder {
                Some(get_builder_address())
            } else {
                None
            };

            let to_addr = Address::from_str(to).unwrap();
            let receipt = wallet
                .transfer(to_addr, U256::from(*amount), builder_addr)
                .await
                .unwrap();
            println!("{:?}", receipt);
        }
        Commands::Balance => {
            let result = wallet.balance().await.unwrap();
            println!("{}", result);
        }
        Commands::TransferErc20 {
            contract_address,
            amount,
            to,
            guaranteed_by_builder,
        } => {
            let builder_addr = if *guaranteed_by_builder {
                Some(get_builder_address())
            } else {
                None
            };
            let to_addr = Address::from_str(to).unwrap();
            let contract_addr = Address::from_str(contract_address).unwrap();
            let receipt = wallet
                .transfer_erc20(contract_addr, to_addr, U256::from(*amount), builder_addr)
                .await
                .unwrap();
            println!("{:?}", receipt);
        }
        Commands::BalanceErc20 { contract_address } => {
            let contract_addr = Address::from_str(contract_address).unwrap();
            let balance = wallet.balance_erc20(contract_addr).await.unwrap();
            println!("{:?}", balance);
        }
        Commands::MintErc20 {
            contract_address,
            amount,
            to,
            guaranteed_by_builder,
        } => {
            let builder_addr = if *guaranteed_by_builder {
                Some(get_builder_address())
            } else {
                None
            };
            let to_addr = Address::from_str(to).unwrap();
            let contract_addr = Address::from_str(contract_address).unwrap();
            let receipt = wallet
                .mint_erc20(contract_addr, to_addr, U256::from(*amount), builder_addr)
                .await
                .unwrap();
            println!("{:?}", receipt);
        }
    }
}

#[cfg(test)]
mod test {
    use assert_cmd::Command;
    use ethers::utils::Anvil;

    static MNEMONIC: &str = "test test test test test test test test test test test junk";
    #[test]
    fn test_bin_balance() -> anyhow::Result<()> {
        let anvil = Anvil::new().chain_id(1u64).spawn();
        dbg!(env!("CARGO_PKG_NAME"));
        dbg!(env!("CARGO_BIN_NAME"));
        let path = assert_cmd::cargo::cargo_bin(env!("CARGO_PKG_NAME"));
        dbg!(path);
        let mut cmd = Command::cargo_bin(env!("CARGO_PKG_NAME")).unwrap();
        cmd.env("MNEMONIC", MNEMONIC)
            .env("ROLLUP_RPC_URL", anvil.endpoint())
            .arg("balance")
            .assert()
            .success();

        Ok(())
    }

    #[test]
    fn test_bin_transfer() -> anyhow::Result<()> {
        let anvil = Anvil::new().chain_id(1u64).spawn();
        let addr = "0xdcfd71e8bc0fef04efab73bd0d79e3b1106b4067";

        let mut cmd = Command::cargo_bin(env!("CARGO_PKG_NAME")).unwrap();
        cmd.env("MNEMONIC", MNEMONIC)
            .env("ROLLUP_RPC_URL", anvil.endpoint())
            .arg("transfer")
            .arg("--amount")
            .arg("1")
            .arg("--to")
            .arg(addr)
            .assert()
            .success();

        Ok(())
    }
}
