use anyhow::Result;
use commit::{self, Commitment, Committable, RawCommitmentBuilder};
use ethers::core::k256::ecdsa::SigningKey;
use ethers::prelude::*;
use ethers::providers::{Http, Provider};
use ethers::signers::coins_bip39::English;
use ethers::signers::MnemonicBuilder;
use lazy_static::lazy_static;

lazy_static! {
    static ref MAGIC_BYTES: [u8; 32] =
        RawCommitmentBuilder::<DummyCommittable>::new("espresso-builder-zNC8sXSk5Yl6Uiu")
            .finalize()
            .into();
}

pub struct EspressoWallet {
    pub client: SignerMiddleware<Provider<Http>, Wallet<SigningKey>>,
}

impl EspressoWallet {
    pub fn new(mnemonic: String, account_index: u32, rollup_url: String) -> Result<Self> {
        let wallet = MnemonicBuilder::<English>::default()
            .phrase(mnemonic.as_str())
            .index(account_index)?
            .build()?;
        let provider = Provider::<Http>::try_from(rollup_url)?;
        let client = SignerMiddleware::new(provider, wallet);
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

struct DummyCommittable;
impl Committable for DummyCommittable {
    fn commit(&self) -> Commitment<Self> {
        unreachable!()
    }

    fn tag() -> String {
        unreachable!()
    }
}
