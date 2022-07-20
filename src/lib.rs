use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};
use near_sdk::collections::{ LookupMap, UnorderedSet};
use near_sdk::json_types::{ U128 };
use near_sdk::serde::{Deserialize, Serialize};
use near_sdk::{
    env, near_bindgen, ext_contract, AccountId, CryptoHash, PanicOnDefault, PromiseOrValue, Timestamp, Gas, BorshStorageKey
};

use near_contract_standards::non_fungible_token::core::NonFungibleTokenReceiver;

use crate::external::*;

mod external;
mod nft_callbacks;

near_sdk::setup_alloc!();

pub type TimestampSec = u64;
pub type TokenId = String;

const GAS_FOR_FT_TRANSFER: Gas = 5_000_000_000_000;
const GAS_FOR_NFT_TRANSFER: Gas = 20_000_000_000_000;

#[near_bindgen]
#[derive(BorshDeserialize, BorshSerialize, PanicOnDefault)]
pub struct Contract {
    //contract owner
    pub owner_id: AccountId,

    //keeps track of all the token IDs for a given account
    pub user_info: LookupMap<AccountId, UnorderedSet<TokenId>>,
	
	pub token_info: LookupMap<TokenId, TimestampSec>,

    pub nft_contract_id: TokenId,
	
	pub ft_token_id: TokenId,

    pub token_reward: u64,
	
	pub time_epoch: u64,
	
	pub total_staked: u64,
}

/// Helper structure for keys of the persistent collections.
#[derive(BorshStorageKey,BorshSerialize)]
pub enum StorageKey {
    UserInfo,
    UserInfoInner { account_id_hash: CryptoHash },
	TokenInfo,
    TokenInfoInner { account_id_hash: CryptoHash },
}

#[near_bindgen]
impl Contract {
    #[init]
    pub fn new(owner_id: AccountId) -> Self {
        //create a variable of type Self with all the fields initialized. 
        Self {
            owner_id: owner_id.into(),
			user_info: LookupMap::new(StorageKey::UserInfo),
			token_info: LookupMap::new(StorageKey::TokenInfo),
			nft_contract_id: "kaizofighters.tenk.near".to_string(),
			ft_token_id: "dojo.near".to_string(),
			token_reward: 3,
			time_epoch: 604800,
			total_staked: 0
        }
    }
	
	pub fn set_nft_contract(&mut self, contract_id: AccountId) {
        self.assert_owner();
        self.nft_contract_id = contract_id;
    }
	
	pub fn set_ft_contract(&mut self, contract_id: AccountId) {
        self.assert_owner();
        self.ft_token_id = contract_id;
    }
	
	pub fn set_owner(&mut self, owner_id: AccountId) {
        self.assert_owner();
        self.owner_id = owner_id;
    }
	
	pub fn get_status(&self ) -> u64{
        self.total_staked
    }
	
	pub fn get_amount_by_owner(&self, account_id: AccountId ) -> Vec<TokenId>{
		let tokens = self.user_info.get(&account_id);
		if let Some(tokens) = tokens {
			tokens.to_vec()
		} else {
			return vec![]; 
		}
    }
	
	pub fn get_staked_time(&self, token_ids: Vec<TokenId> ) -> Vec<TimestampSec>{
		let mut tmp = vec![];
        for i in 0..token_ids.len() {
            tmp.push(self.token_info.get(&token_ids[i]).unwrap_or(0));
        }
        tmp 
    }
	
	pub fn set_time_epoch(&mut self, period: u64) {
        self.assert_owner();
        self.time_epoch = period;
    }
	
	pub fn set_token_reward(&mut self, amount: u64) {
        self.assert_owner();
        self.token_reward = amount;
    }
	
	pub(crate) fn stake(&mut self, account_id: AccountId, token_id: TokenId) {
        let mut user_infos = self.user_info.get(&account_id).unwrap_or_else(|| {
		  UnorderedSet::new(
			b"D".to_vec(),
		  )
		});
		user_infos.insert(&token_id);
		
		self.user_info.insert(&account_id, &user_infos);
		self.token_info.insert(&token_id, &self.to_sec(env::block_timestamp()));
		self.total_staked += 1;
    }
	
	#[payable]
	pub fn unstake(&mut self, token_id: TokenId) {
		self.assert_at_least_one_yocto();
		let account_id = env::signer_account_id();
        let mut user_infos = self.user_info.get(&account_id).expect("No Staked NFTs");
		
		user_infos.remove(&token_id);
		
		ext_non_fungible_token::nft_transfer(
            account_id.clone(),
            token_id,
            None,
            None,
            &self.nft_contract_id,
            1,
            GAS_FOR_NFT_TRANSFER,
        );
		
		self.total_staked -= 1;
		if user_infos.is_empty() {
			self.user_info.remove(&account_id);
		} else {
			self.user_info.insert(&account_id, &user_infos);
		}
    }
	
	#[payable]
	pub fn claim(&mut self) -> bool {
		self.assert_at_least_one_yocto();
		let account_id = env::signer_account_id();
		let staked_tokens = self.user_info.get(&account_id).expect("No Staked NFTs");
		
		let reward = self.get_reward(account_id.clone());
		
		if reward != 0 {
			let current_stimetamp = self.to_sec(env::block_timestamp());
		
			for k in staked_tokens.iter() {
				let key = k.clone();
				self.token_info.insert(&key, &current_stimetamp);
			}
			
			let token_amount = reward * 10u64.pow(18);
			ext_fungible_token::ft_transfer(
                account_id.clone(),
                U128(token_amount as u128),
                None,
                &self.ft_token_id,
                1,
                GAS_FOR_FT_TRANSFER,
            );
			true
		} else {
			false
		}
		
	}
	
	pub(crate) fn assert_owner(&self) {
		assert_eq!(
			&env::predecessor_account_id(),
			&self.owner_id,
			"Owner's method"
		);
	}
	
	pub(crate) fn assert_at_least_one_yocto(&self) {
		assert!(
			env::attached_deposit() >= 1,
			"Requires attached deposit of at least 1 yoctoNEAR",
		)
	}
	
	pub fn get_reward(&self, from: AccountId) -> u64{
		let staked_tokens = self.user_info.get(&from);
			
		if let Some(staked_tokens) = staked_tokens {
			let mut total_perpetual = 0;
			
			for k in staked_tokens.iter() {
				let key = k.clone();
				let token_created_time = self.token_info.get(&key).unwrap();
				total_perpetual += self.stakedtime_to_amount( token_created_time );
			}
			
			self.token_reward * total_perpetual as u64
			
		} else {
			0
		}
	}
	
	pub(crate) fn stakedtime_to_amount(&self, staked_time: u64) -> u64 {
		let current_stimetamp = self.to_sec(env::block_timestamp());
		
		( current_stimetamp - staked_time ) / self.time_epoch
	}

	pub(crate) fn to_sec(&self, timestamp: Timestamp) -> TimestampSec {
		(timestamp / 10u64.pow(9)) as u64
	}

}