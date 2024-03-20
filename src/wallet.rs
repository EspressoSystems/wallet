use std::sync::Arc;

use anyhow::Result;
use commit::{self, Commitment, Committable, RawCommitmentBuilder};
use ethers::core::k256::ecdsa::SigningKey;
use ethers::prelude::*;
use ethers::signers::coins_bip39::English;
use ethers::signers::MnemonicBuilder;
use lazy_static::lazy_static;

lazy_static! {
    static ref MAGIC_BYTES: [u8; 32] =
        RawCommitmentBuilder::<DummyCommittable>::new("espresso-builder-zNC8sXSk5Yl6Uiu")
            .finalize()
            .into();
}

// https://github.com/gakonst/ethers-rs/blob/master/examples/transactions/examples/transfer_erc20.rs
abigen!(
    Erc20Contract,
    r#"[
        function balanceOf(address account) external view returns (uint256)
        function decimals() external view returns (uint8)
        function symbol() external view returns (string memory)
        function transfer(address to, uint256 amount) external returns (bool)
        function mint(address to, uint256 amount) external
        event Transfer(address indexed from, address indexed to, uint256 value)
    ]"#,
);

pub struct EspressoWallet {
    pub client: Arc<SignerMiddleware<Provider<Http>, Wallet<SigningKey>>>,
}

impl EspressoWallet {
    pub fn new(mnemonic: String, account_index: u32, rollup_url: String) -> Result<Self> {
        let wallet = MnemonicBuilder::<English>::default()
            .phrase(mnemonic.as_str())
            .index(account_index)?
            .build()?;
        let provider = Provider::<Http>::try_from(rollup_url)?;
        let client = Arc::new(SignerMiddleware::new(provider, wallet));
        Ok(Self { client })
    }

    pub async fn balance(&self) -> Result<U256> {
        let addr = self.client.address();
        let balance = self.client.get_balance(addr, None).await?;
        Ok(balance)
    }

    pub async fn transfer(
        &self,
        to: Address,
        amount: U256,
        builder: Option<Address>,
    ) -> Result<TransactionReceipt> {
        let gas_price = self.client.get_gas_price().await?;
        let nonce = self.get_account_nounce().await?;
        let chain_id = self.client.get_chainid().await?.as_u64();
        let mut tx_request = TransactionRequest {
            from: Some(self.client.address()),
            to: Some(to.into()),
            value: Some(amount),
            gas_price: Some(gas_price),
            nonce: Some(nonce),
            chain_id: Some(chain_id.into()),
            ..Default::default()
        };

        if let Some(b) = builder {
            let mut extra_data = [0u8; 52];
            extra_data[0..32].copy_from_slice(MAGIC_BYTES.as_slice());
            extra_data[32..52].copy_from_slice(b.as_bytes());
            tx_request = tx_request.data(extra_data);
        };
        let receipt = self.send_transaction(tx_request).await?;
        Ok(receipt)
    }

    pub async fn transfer_erc20(
        &self,
        contract_addr: Address,
        to: Address,
        amount: U256,
        builder: Option<Address>,
    ) -> Result<TransactionReceipt> {
        let contract = Erc20Contract::new(contract_addr, self.client.clone());
        let decimals = contract.decimals().call().await?;
        let decimal_amount = amount * U256::exp10(decimals as usize);
        let mut calldata = contract
            .transfer(to, decimal_amount)
            .calldata()
            .unwrap_or([].into());
        if let Some(builder_addr) = builder {
            calldata = append_calldata_with_builder_address(calldata, builder_addr);
        }
        let chain_id = self.client.get_chainid().await?.as_u64();
        let nonce = self.get_account_nounce().await?;

        let tx_request = TransactionRequest {
            from: Some(self.client.address()),
            to: Some(contract_addr.into()),
            data: Some(calldata),
            chain_id: Some(chain_id.into()),
            nonce: Some(nonce),
            ..Default::default()
        };

        let receipt = self.send_transaction(tx_request).await?;
        Ok(receipt)
    }

    pub async fn balance_erc20(&self, contract_addr: Address) -> Result<U256> {
        let contract = Erc20Contract::new(contract_addr, self.client.clone());
        let balance = contract.balance_of(self.client.address()).call().await?;
        Ok(balance)
    }

    pub async fn mint_erc20(
        &self,
        contract_addr: Address,
        to: Address,
        amount: U256,
        builder: Option<Address>,
    ) -> Result<TransactionReceipt> {
        let contract = Erc20Contract::new(contract_addr, self.client.clone());
        let decimals = contract.decimals().call().await?;
        let decimal_amount = amount * U256::exp10(decimals as usize);
        let mut calldata = contract
            .mint(to, decimal_amount)
            .calldata()
            .unwrap_or([].into());
        if let Some(builder_addr) = builder {
            calldata = append_calldata_with_builder_address(calldata, builder_addr);
        }
        let chain_id = self.client.get_chainid().await?.as_u64();
        let nonce = self.get_account_nounce().await?;

        let tx_request = TransactionRequest {
            from: Some(self.client.address()),
            to: Some(contract_addr.into()),
            data: Some(calldata),
            chain_id: Some(chain_id.into()),
            nonce: Some(nonce),
            ..Default::default()
        };

        let receipt = self.send_transaction(tx_request).await?;
        Ok(receipt)
    }

    #[inline]
    async fn send_transaction(&self, tx: TransactionRequest) -> Result<TransactionReceipt> {
        let pending_tx = self.client.send_transaction(tx, None);
        let receipt = pending_tx.await?.await?.unwrap();
        Ok(receipt)
    }

    #[inline]
    async fn get_account_nounce(&self) -> Result<U256> {
        let address = self.client.address();
        let nonce = self.client.get_transaction_count(address, None).await?;
        Ok(nonce)
    }
}

fn append_calldata_with_builder_address(calldata: Bytes, builder: Address) -> Bytes {
    let mut extra_data = [0u8; 52];
    extra_data[0..32].copy_from_slice(MAGIC_BYTES.as_slice());
    extra_data[32..52].copy_from_slice(builder.as_bytes());

    let mut data_vec = calldata.to_vec();
    data_vec.extend_from_slice(&extra_data);

    Bytes::from(data_vec)
}

struct DummyCommittable;
impl Committable for DummyCommittable {
    fn commit(&self) -> Commitment<Self> {
        unreachable!()
    }

    fn tag() -> String {
        unreachable!()
    }
}

#[cfg(test)]
mod test {
    use crate::contracts::{simple_erc20::simple_erc20::SimpleERC20, weth9::weth9::WETH9};

    use super::*;
    use ethers::utils::Anvil;

    static MNEMONIC: &str = "test test test test test test test test test test test junk";
    // initial balance as configured in Anvil
    const INITIAL_BALANCE: u128 = 10000000000000000000000u128;

    #[test]
    fn test_new_wallet() -> anyhow::Result<()> {
        let anvil = Anvil::new().spawn();
        EspressoWallet::new(MNEMONIC.into(), 1, anvil.endpoint())?;

        Ok(())
    }
    #[async_std::test]
    async fn test_balance() -> anyhow::Result<()> {
        let anvil = Anvil::new().spawn();
        let wallet = EspressoWallet::new(MNEMONIC.into(), 1, anvil.endpoint()).unwrap();
        let balance = wallet.balance().await?;
        assert_eq!(U256::from(INITIAL_BALANCE), balance);

        Ok(())
    }
    #[async_std::test]
    async fn test_deploy() -> anyhow::Result<()> {
        // use wallet default chain_id
        // should this be an option passed to wallet?
        let anvil = Anvil::new().chain_id(1u64).spawn();
        let wallet = EspressoWallet::new(MNEMONIC.into(), 0, anvil.endpoint())?;

        let provider = wallet.client;
        let _contract = WETH9::deploy(provider, ()).unwrap().send().await?;

        Ok(())
    }

    #[async_std::test]
    async fn test_transfer() -> anyhow::Result<()> {
        let anvil = Anvil::new().chain_id(1u64).spawn();
        let wallet = EspressoWallet::new(MNEMONIC.into(), 0, anvil.endpoint())?;
        let addr = Address::random();
        let _receipt = wallet
            .transfer(addr, U256::from(1000000000000000u128), None)
            .await?;
        let balance = wallet.balance().await?;
        assert!(balance < U256::from(INITIAL_BALANCE));
        let _receipt = wallet
            .transfer(
                addr,
                U256::from(1000000000000000u128),
                Some(Address::random()),
            )
            .await?;
        let new_balance = wallet.balance().await?;
        assert!(new_balance < balance);
        Ok(())
    }

    #[async_std::test]
    async fn test_erc20() -> anyhow::Result<()> {
        let anvil = Anvil::new().chain_id(1u64).spawn();
        let wallet = EspressoWallet::new(MNEMONIC.into(), 0, anvil.endpoint())?;

        let erc20_contract = SimpleERC20::deploy(wallet.client.clone(), ())
            .unwrap()
            .send()
            .await?;

        let erc20_addr = erc20_contract.address();

        // The extra bytes appended to calldata shouldn't affect the
        // transaction execution.
        let builder_addr = Address::random();
        let amount = U256::from(1000);

        wallet
            .mint_erc20(erc20_addr, wallet.client.address(), amount, None)
            .await?;
        wallet
            .mint_erc20(
                erc20_addr,
                wallet.client.address(),
                amount,
                Some(builder_addr),
            )
            .await?;

        let balance = wallet.balance_erc20(erc20_addr).await?;
        assert_eq!(balance, U256::from(2000));

        let to_addr = Address::random();
        wallet
            .transfer_erc20(erc20_addr, to_addr, amount, None)
            .await?;
        wallet
            .transfer_erc20(erc20_addr, to_addr, amount, Some(builder_addr))
            .await?;

        let balance = wallet.balance_erc20(erc20_addr).await?;
        assert_eq!(balance, U256::from(0));
        Ok(())
    }
}
