use anyhow::Result;
use commit::{self, Commitment, Committable, RawCommitmentBuilder};
use contract_bindings::simple_token::SimpleToken as Erc20Contract;
use ethers::core::k256::ecdsa::SigningKey;
use ethers::prelude::*;
use ethers::signers::coins_bip39::English;
use ethers::signers::MnemonicBuilder;
use lazy_static::lazy_static;
use std::sync::Arc;
use std::time::Duration;

pub const DEV_MNEMONIC: &str = "test test test test test test test test test test test junk";

lazy_static! {
    static ref MAGIC_BYTES: [u8; 32] =
        RawCommitmentBuilder::<DummyCommittable>::new("espresso-builder-zNC8sXSk5Yl6Uiu")
            .finalize()
            .into();
}

pub struct EspressoWallet {
    pub client: Arc<SignerMiddleware<Provider<Http>, Wallet<SigningKey>>>,
}

impl EspressoWallet {
    pub fn new(
        mnemonic: impl AsRef<str>,
        account_index: u32,
        rollup_url: String,
        chain_id: u64,
    ) -> Result<Self> {
        let wallet = MnemonicBuilder::<English>::default()
            .phrase(mnemonic.as_ref())
            .index(account_index)?
            .build()?
            .with_chain_id(chain_id);

        let interval = if cfg!(test) {
            // Poll every 10ms in tests to make tests complete faster.
            Duration::from_millis(10)
        } else {
            // Currently the sequencer has about a 1 - 3 sec block time.
            Duration::from_secs(3)
        };
        let provider = Provider::<Http>::try_from(rollup_url)?.interval(interval);
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
        let tx_request = TransactionRequest {
            from: Some(self.client.address()),
            to: Some(to.into()),
            value: Some(amount),
            gas_price: Some(gas_price),
            nonce: Some(nonce),
            chain_id: Some(chain_id.into()),
            ..Default::default()
        };

        let mut calldata = Bytes::new();
        if let Some(b) = builder {
            calldata = append_calldata_with_builder_address(calldata, b);
        };
        let tx_request = tx_request.data(calldata);
        let receipt = self.send_transaction(tx_request).await?;
        Ok(receipt)
    }

    pub async fn deploy_erc20(
        &self,
        name: impl AsRef<str>,
        symbol: impl AsRef<str>,
    ) -> Result<Erc20Contract<SignerMiddleware<Provider<Http>, Wallet<SigningKey>>>> {
        let contract = Erc20Contract::deploy(
            self.client.clone(),
            (
                name.as_ref().to_string(),
                symbol.as_ref().to_string(),
                18u8, /*decimals*/
            ),
        )?
        .send()
        .await?;
        Ok(contract)
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
        to: Option<Address>,
        amount: Option<U256>,
        builder: Option<Address>,
    ) -> Result<TransactionReceipt> {
        let to = to.unwrap_or(self.client.address());
        let contract = Erc20Contract::new(contract_addr, self.client.clone());
        let decimals = contract.decimals().call().await?;
        let amount = amount.unwrap_or(U256::from(1000));
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
        let interval = if cfg!(test) {
            Duration::from_millis(500)
        } else {
            Duration::from_secs(1)
        };
        let pending_tx = self.client.send_transaction(tx, None);
        let receipt = pending_tx
            .await?
            .retries(10)
            .interval(interval)
            .await?
            .expect("Cannot get the receipt");
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
    extra_data[32..52].copy_from_slice(builder.as_fixed_bytes());

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
    use super::*;
    use contract_bindings::simple_token::SimpleToken;
    use ethers::utils::Anvil;

    // initial balance as configured in Anvil
    const INITIAL_BALANCE: u128 = 10000000000000000000000u128;

    impl EspressoWallet {
        fn for_test(rollup_url: String) -> Self {
            Self::new(DEV_MNEMONIC, 1, rollup_url, 1).unwrap()
        }
    }

    #[test]
    fn test_new_wallet() -> anyhow::Result<()> {
        let anvil = Anvil::new().chain_id(1u64).spawn();
        EspressoWallet::for_test(anvil.endpoint());

        Ok(())
    }
    #[async_std::test]
    async fn test_balance() -> anyhow::Result<()> {
        let anvil = Anvil::new().chain_id(1u64).spawn();
        let wallet = EspressoWallet::for_test(anvil.endpoint());
        let balance = wallet.balance().await?;
        assert_eq!(U256::from(INITIAL_BALANCE), balance);

        Ok(())
    }
    #[async_std::test]
    async fn test_deploy() -> anyhow::Result<()> {
        // use wallet default chain_id
        // should this be an option passed to wallet?
        let anvil = Anvil::new().chain_id(1u64).spawn();
        let wallet = EspressoWallet::for_test(anvil.endpoint());

        let provider = wallet.client;
        let _contract = SimpleToken::deploy(
            provider,
            ("name".to_string(), "symbol".to_string(), U256::from(18)),
        )
        .unwrap()
        .send()
        .await?;

        Ok(())
    }

    #[async_std::test]
    async fn test_transfer() -> anyhow::Result<()> {
        let anvil = Anvil::new().chain_id(1u64).spawn();
        let wallet = EspressoWallet::for_test(anvil.endpoint());

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
    async fn test_erc20_mint_max_value() -> anyhow::Result<()> {
        let anvil = Anvil::new().chain_id(1u64).spawn();
        let wallet = EspressoWallet::for_test(anvil.endpoint());

        let erc20_contract = SimpleToken::deploy(
            wallet.client.clone(),
            ("name".to_string(), "symbol".to_string(), U256::from(18)),
        )
        .unwrap()
        .send()
        .await?;

        let contract_addr = erc20_contract.address();

        // The extra bytes appended to calldata shouldn't affect the
        // transaction execution.
        let amount = U256::from(u128::MAX);

        wallet
            .mint_erc20(
                contract_addr,
                Some(wallet.client.address()),
                Some(amount),
                None,
            )
            .await
            .unwrap_err();

        Ok(())
    }

    #[async_std::test]
    async fn test_erc20() -> anyhow::Result<()> {
        let anvil = Anvil::new().chain_id(1u64).spawn();
        let wallet = EspressoWallet::for_test(anvil.endpoint());

        let erc20_contract = SimpleToken::deploy(
            wallet.client.clone(),
            ("name".to_string(), "symbol".to_string(), U256::from(18)),
        )
        .unwrap()
        .send()
        .await?;

        let contract_addr = erc20_contract.address();

        // The extra bytes appended to calldata shouldn't affect the
        // transaction execution.
        let builder_addr = Address::random();
        let amount = U256::from(1000);
        let initial_balance = wallet.balance_erc20(contract_addr).await?;

        wallet
            .mint_erc20(contract_addr, None, Some(amount), None)
            .await?;
        wallet
            .mint_erc20(contract_addr, None, Some(amount), Some(builder_addr))
            .await?;

        let decimals = erc20_contract.decimals().call().await?;
        let decimal_amount = amount * U256::exp10(decimals as usize);
        let balance = wallet.balance_erc20(contract_addr).await?;
        assert_eq!(
            balance,
            decimal_amount
                .checked_mul(2.into())
                .unwrap()
                .checked_add(initial_balance)
                .unwrap()
        );

        let to_addr = Address::random();
        wallet
            .transfer_erc20(contract_addr, to_addr, amount, None)
            .await?;
        wallet
            .transfer_erc20(contract_addr, to_addr, amount, Some(builder_addr))
            .await?;

        let balance = wallet.balance_erc20(contract_addr).await?;
        assert_eq!(balance, initial_balance);
        Ok(())
    }

    #[async_std::test]
    async fn test_deploy_contract_with_builder() -> anyhow::Result<()> {
        let anvil = Anvil::new().chain_id(1u64).spawn();
        let wallet = EspressoWallet::for_test(anvil.endpoint());

        let erc20_contract = SimpleToken::deploy(
            wallet.client.clone(),
            ("name".to_string(), "symbol".to_string(), U256::from(18)),
        )
        .unwrap();

        let data = erc20_contract.deployer.tx.data().unwrap();
        let data = append_calldata_with_builder_address(data.clone(), Address::random());
        let new_tx = erc20_contract.data(data);
        let contract = new_tx.send().await?;
        let contract_addr = contract.address();

        let amount = U256::from(1000);
        let initial_balance = wallet.balance_erc20(contract_addr).await?;

        wallet
            .mint_erc20(contract_addr, None, Some(amount), None)
            .await?;
        wallet
            .mint_erc20(contract_addr, None, Some(amount), Some(Address::random()))
            .await?;

        let decimals = contract.decimals().call().await?;
        let decimal_amount = amount * U256::exp10(decimals as usize);
        let balance = wallet.balance_erc20(contract_addr).await?;
        assert_eq!(
            balance,
            decimal_amount
                .checked_mul(2.into())
                .unwrap()
                .checked_add(initial_balance)
                .unwrap()
        );
        Ok(())
    }
}
