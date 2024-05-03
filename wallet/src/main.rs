mod builder;
mod wallet;
use builder::get_builder_address;
use clap::Subcommand;
use clap_serde_derive::{
    clap::{self, Parser},
    ClapSerde,
};
use ethers::{
    core::rand::thread_rng,
    providers::{Http, Middleware, Provider},
    signers::{
        coins_bip39::{English, Mnemonic},
        MnemonicBuilder, Signer as _,
    },
    types::{Address, U256},
};
use serde::{Deserialize, Serialize};
use std::fs;
use url::Url;
use wallet::EspressoWallet;
use wallet::DEV_MNEMONIC;

#[derive(Parser)]
#[command(author, version, about)]
struct Args {
    /// Config file
    #[arg(short, long = "config", default_value = "config.toml")]
    config_path: std::path::PathBuf,

    /// Rest of arguments
    #[command(flatten)]
    pub config: <Config as ClapSerde>::Opt,
}

#[derive(ClapSerde, Debug, Deserialize, Serialize)]
pub struct Config {
    #[clap(long, env = "MNEMONIC", default_value = DEV_MNEMONIC)]
    #[serde(alias = "mnemonic", alias = "MNEMONIC")]
    pub mnemonic: String,

    /// The url for the rollup rpc.
    #[clap(long, env = "ROLLUP_RPC_URL")]
    #[default(Url::parse("http://localhost:8545").unwrap())]
    rollup_rpc_url: Url,

    /// The url for fetching the builder address.
    #[clap(long, env = "BUILDER_URL")]
    builder_url: Option<Url>,

    /// The builder address. Lower priority than `builder_url`.
    #[clap(long, env = "BUILDER_ADDRESS")]
    builder_addr: Option<Address>,

    #[clap(long, env = "ACCOUNT_INDEX", default_value = "0")]
    account_index: u32,

    #[command(subcommand)]
    #[serde(skip)]
    commands: Commands,
}

#[derive(Default, Subcommand, Debug, Deserialize, Serialize)]
enum Commands {
    /// Initialize the config file with a new mnemonic.
    Init,
    /// Transfer Eth to another address.
    Transfer {
        /// hex string of the target address
        #[clap(long)]
        to: Address,

        #[clap(long)]
        amount: u64,

        #[clap(long, default_value_t = false)]
        guaranteed_by_builder: bool,
    },
    /// Deploy ERC20 token.
    DeployErc20 {
        #[clap(long, default_value = "TestToken")]
        name: String,

        #[clap(long, default_value = "TOK")]
        symbol: String,
    },
    /// Transfer ERC20 tokens to another address.
    TransferErc20 {
        #[clap(long)]
        contract_address: Address,

        #[clap(long)]
        amount: u64,

        /// hex string of the target address
        #[clap(long)]
        to: Address,

        #[clap(long, default_value_t = false)]
        guaranteed_by_builder: bool,
    },
    /// Check Eth balance.
    #[default]
    Balance,
    /// Check ERC20 token balance.
    BalanceErc20 {
        #[clap(long)]
        contract_address: Address,
    },
    /// Mint ERC20 tokens to own account.
    MintErc20 {
        #[clap(long)]
        contract_address: Address,

        #[clap(long)]
        amount: Option<u64>,

        /// hex string of the target address, default to own address
        #[clap(long)]
        to: Option<Address>,

        #[clap(long, default_value_t = false)]
        guaranteed_by_builder: bool,
    },
}

fn exit_err(msg: impl AsRef<str>, err: impl core::fmt::Display) -> ! {
    eprintln!("{}: {err}", msg.as_ref());
    std::process::exit(1);
}

#[async_std::main]
async fn main() -> anyhow::Result<()> {
    let mut cli = Args::parse();
    // Get config file
    let config = if let Ok(f) = fs::read_to_string(&cli.config_path) {
        // parse toml
        match toml::from_str::<Config>(&f) {
            Ok(config) => config.merge(&mut cli.config),
            Err(err) => {
                // This is a user error print the hopefully helpful error
                // message without backtrace and exit.
                exit_err("Error in configuration file", err);
            }
        }
    } else {
        // If there is no config file return only config parsed from clap
        Config::from(&mut cli.config)
    };

    // Run the init command first because config values required by other
    // commands are not present.
    if let Commands::Init = config.commands {
        let mut config = toml::from_str::<Config>(include_str!("../../config.toml.default"))?;
        // Generate a new mnemonic for the user.
        let mnemonic = Mnemonic::<English>::new(&mut thread_rng());
        config.mnemonic = mnemonic.to_phrase();
        let wallet = MnemonicBuilder::<English>::default()
            .phrase(config.mnemonic.as_ref())
            .build()?;
        println!("Address of new wallet: {:#x}", wallet.address());

        // Save config file to cli.config_path
        fs::write(&cli.config_path, toml::to_string(&config)?)?;
        println!("Config file saved to {}", cli.config_path.display());
        return Ok(());
    }

    let provider = Provider::<Http>::try_from(&config.rollup_rpc_url.to_string())?;
    let id = provider
        .get_chainid()
        .await
        .unwrap_or_else(|err| exit_err("failed to get chain ID from rollup RPC", err));
    let wallet = EspressoWallet::new(
        config.mnemonic,
        config.account_index,
        config.rollup_rpc_url.to_string(),
        id.as_u64(),
    );
    let wallet = wallet.unwrap_or_else(|err| exit_err("failed to create a wallet", err));

    match &config.commands {
        Commands::Transfer {
            to,
            amount,
            guaranteed_by_builder,
        } => {
            let builder_addr = maybe_get_builder_addr(
                guaranteed_by_builder,
                config.builder_url,
                config.builder_addr,
            )
            .await;
            let receipt = wallet
                .transfer(*to, U256::from(*amount), builder_addr)
                .await?;
            println!("{:?}", receipt);
        }
        Commands::Balance => {
            let result = wallet.balance().await?;
            println!("{}", result);
        }
        Commands::DeployErc20 { name, symbol } => {
            let contract = wallet.deploy_erc20(name, symbol).await?;
            println!("ERC20 token deployed at {:#x}", contract.address());
        }
        Commands::TransferErc20 {
            contract_address,
            amount,
            to,
            guaranteed_by_builder,
        } => {
            let builder_addr = maybe_get_builder_addr(
                guaranteed_by_builder,
                config.builder_url,
                config.builder_addr,
            )
            .await;
            let receipt = wallet
                .transfer_erc20(*contract_address, *to, U256::from(*amount), builder_addr)
                .await?;
            println!("{:?}", receipt);
        }
        Commands::BalanceErc20 { contract_address } => {
            let balance = wallet.balance_erc20(*contract_address).await?;
            println!("{:?}", balance.to_string());
        }
        Commands::MintErc20 {
            contract_address,
            amount,
            to,
            guaranteed_by_builder,
        } => {
            let builder_addr = maybe_get_builder_addr(
                guaranteed_by_builder,
                config.builder_url,
                config.builder_addr,
            )
            .await;
            let receipt = wallet
                .mint_erc20(*contract_address, *to, amount.map(U256::from), builder_addr)
                .await;
            match receipt {
                Ok(r) => println!("{:?}", r),
                Err(err) => exit_err("failed to get receipt", err),
            }
        }
        _ => {} // The init command is handled before this match.
    };
    Ok(())
}

async fn maybe_get_builder_addr(
    guaranteed_by_builder: &bool,
    builder_url: Option<Url>,
    builder_addr: Option<Address>,
) -> Option<Address> {
    if *guaranteed_by_builder {
        if let Some(url) = builder_url {
            return Some(get_builder_address(url).await);
        };
        builder_addr
    } else {
        None
    }
}

#[cfg(test)]
mod test {
    use crate::wallet::DEV_MNEMONIC;
    use assert_cmd::Command;
    use ethers::{types::Address, utils::Anvil};

    #[test]
    fn test_bin_balance() -> anyhow::Result<()> {
        let anvil = Anvil::new().chain_id(1u64).spawn();
        let mut cmd = Command::cargo_bin(env!("CARGO_PKG_NAME"))?;
        cmd.env("MNEMONIC", DEV_MNEMONIC)
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
        let mut cmd = Command::cargo_bin(env!("CARGO_PKG_NAME"))?;
        cmd.env("MNEMONIC", DEV_MNEMONIC)
            .env("ROLLUP_RPC_URL", anvil.endpoint())
            .env("BUILDER_ADDRESS", valid_builder_address)
            .arg("transfer")
            .arg("--amount")
            .arg("1")
            .arg("--to")
            .arg(format!("{:#x}", Address::random()))
            .arg("--guaranteed-by-builder")
            .assert()
            .success();

        Ok(())
    }

    #[test]
    fn test_generate_config_file() -> anyhow::Result<()> {
        let tmpdir = tempfile::tempdir()?;
        let config_path = tmpdir.path().join("config.toml");

        assert!(!config_path.exists());

        let mut cmd = Command::cargo_bin(env!("CARGO_PKG_NAME"))?;
        cmd.arg("-c")
            .arg(&config_path)
            .arg("init")
            .assert()
            .success();

        assert!(config_path.exists());

        let anvil = Anvil::new().chain_id(1u64).spawn();

        // Check that we can query the balance with the config file.
        let mut cmd = Command::cargo_bin(env!("CARGO_PKG_NAME"))?;
        cmd.arg("-c")
            .arg(&config_path)
            // Overwrite the rpc value in the config file so that we can get a response.
            .arg("--rollup-rpc-url")
            .arg(anvil.endpoint())
            .arg("balance")
            .assert()
            .success();

        Ok(())
    }
}
