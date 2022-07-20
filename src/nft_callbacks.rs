use crate::*;

// Receiving NFTs
#[near_bindgen]
impl NonFungibleTokenReceiver for Contract {
    fn nft_on_transfer(
        &mut self,
        sender_id: AccountId,
        previous_owner_id: AccountId,
        token_id: TokenId,
        msg: String,
    ) -> PromiseOrValue<bool> {
        let nft_contract_id = env::predecessor_account_id();
        let signer_id = env::signer_account_id();

        assert_ne!(
            nft_contract_id, signer_id,
            "Staking: nft_on_transfer should only be called via cross-contract call"
        );
		
		assert_eq!(
            nft_contract_id, self.nft_contract_id,
            "Staking: only accept right nfts"
        );

        assert_eq!(
            previous_owner_id,
            signer_id,
            "Staking: owner_id should be signer_id"
        );

        self.stake(sender_id,token_id);
        
		PromiseOrValue::Value(false)
    }
}
