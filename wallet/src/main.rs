mod builder;
mod wallet;

use builder::get_builder_address;
use clap::{Parser, Subcommand};
use ethers::{
    providers::{Http, Middleware, Provider},
    types::{Address, U256},
};
use wallet::EspressoWallet;

#[derive(Parser, Debug)]
pub struct Cli {
    #[clap(long, env = "MNEMONIC")]
    mnemonic: String,

    #[clap(long, env = "ROLLUP_RPC_URL")]
    rollup_rpc_url: String,

    /// The url for fetching the builder address.
    #[clap(long, env = "BUILDER_URL", default_value = "")]
    builder_url: String,

    /// The builder address. Lower priority than `builder_url`.
    #[clap(long, env = "BUILDER_ADDRESS", default_value = "")]
    builder_addr: String,

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
    let provider = Provider::<Http>::try_from(&cli.rollup_rpc_url).unwrap();
    let id = provider.get_chainid().await.unwrap();
    let wallet = EspressoWallet::new(
        cli.mnemonic,
        cli.account_index,
        cli.rollup_rpc_url,
        id.as_u64(),
    );
    if let Err(e) = wallet {
        panic!("failed to create a wallet: {}", e);
    }
    let wallet = wallet.unwrap();

    match &cli.commands {
        Commands::Transfer {
            to,
            amount,
            guaranteed_by_builder,
        } => {
            let builder_addr = if *guaranteed_by_builder {
                if !cli.builder_url.is_empty() {
                    Some(get_builder_address())
                } else if !cli.builder_addr.is_empty() {
                    Some(
                        cli.builder_addr
                            .parse::<Address>()
                            .expect("Invalid builder address."),
                    )
                } else {
                    None
                }
            } else {
                None
            };

            let to_addr = to.parse::<Address>().expect("Invalid to address.");
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
                if !cli.builder_url.is_empty() {
                    Some(get_builder_address())
                } else if !cli.builder_addr.is_empty() {
                    Some(
                        cli.builder_addr
                            .parse::<Address>()
                            .expect("Invalid builder address."),
                    )
                } else {
                    None
                }
            } else {
                None
            };
            let to_addr = to.parse::<Address>().unwrap();
            let contract_addr = contract_address.parse::<Address>().unwrap();
            let receipt = wallet
                .transfer_erc20(contract_addr, to_addr, U256::from(*amount), builder_addr)
                .await
                .unwrap();
            println!("{:?}", receipt);
        }
        Commands::BalanceErc20 { contract_address } => {
            let contract_addr = contract_address.parse::<Address>().unwrap();
            let balance = wallet.balance_erc20(contract_addr).await.unwrap();
            println!("{:?}", balance.to_string());
        }
        Commands::MintErc20 {
            contract_address,
            amount,
            to,
            guaranteed_by_builder,
        } => {
            let builder_addr = if *guaranteed_by_builder {
                if !cli.builder_url.is_empty() {
                    Some(get_builder_address())
                } else if !cli.builder_addr.is_empty() {
                    Some(
                        cli.builder_addr
                            .parse::<Address>()
                            .expect("Invalid builder address."),
                    )
                } else {
                    None
                }
            } else {
                None
            };
            let to_addr = to.parse::<Address>().unwrap();
            let contract_addr = contract_address.parse::<Address>().unwrap();
            let receipt = wallet
                .mint_erc20(contract_addr, to_addr, U256::from(*amount), builder_addr)
                .await;
            match receipt {
                Ok(r) => println!("{:?}", r),
                Err(e) => panic!("got error: {:?}", e),
            }
        }
    }
}

#[cfg(test)]
mod test {
    use assert_cmd::Command;
    use ethers::{types::Address, utils::Anvil};

    static MNEMONIC: &str = "test test test test test test test test test test test junk";
    #[test]
    fn test_bin_balance() -> anyhow::Result<()> {
        let anvil = Anvil::new().chain_id(1u64).spawn();
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
        // Include builder address to catch parsing errors.
        let valid_builder_address = "0x23618e81e3f5cdf7f54c3d65f7fbc0abf5b21e8f";
        let mut cmd = Command::cargo_bin(env!("CARGO_PKG_NAME")).unwrap();
        cmd.env("MNEMONIC", MNEMONIC)
            .env("ROLLUP_RPC_URL", anvil.endpoint())
            .env("BUILDER_ADDRESS", valid_builder_address)
            .arg("transfer")
            .arg("--amount")
            .arg("1")
            .arg("--to")
            .arg(format!("{:x}", Address::random()))
            .assert()
            .success();

        Ok(())
    }
}
