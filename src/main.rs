mod contracts;
mod wallet;

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
    Balance,
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
                // get the builder address
                Some(Address::random())
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
    }
}
