#![cfg_attr(not(feature = "std"), no_std, no_main)]

mod errors;

#[ink::contract]
mod az_token_sale_to_airdrop {
    use crate::errors::AzTokenSaleToAirdropError;
    use ink::{prelude::string::ToString, storage::Mapping};

    // === TYPES ===
    type Result<T> = core::result::Result<T, AzTokenSaleToAirdropError>;

    // === STRUCTS ===
    #[derive(Debug, Clone, scale::Encode, scale::Decode)]
    #[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
    pub struct Config {
        admin: AccountId,
        airdrop: AccountId,
        token: AccountId,
        start: Timestamp,
        end: Timestamp,
        whitelist_duration: Timestamp,
    }

    // === CONTRACT ===
    #[ink(storage)]
    pub struct AzTokenSaleToAirdrop {
        admin: AccountId,
        airdrop: AccountId,
        token: AccountId,
        start: Timestamp,
        end: Timestamp,
        whitelist: Mapping<AccountId, AccountId>,
        whitelist_duration: Timestamp,
    }
    impl AzTokenSaleToAirdrop {
        #[ink(constructor)]
        pub fn new(
            airdrop: AccountId,
            token: AccountId,
            start: Timestamp,
            end: Timestamp,
            whitelist_duration: Timestamp,
        ) -> Self {
            Self {
                admin: Self::env().caller(),
                airdrop,
                token,
                start,
                end,
                whitelist: Mapping::default(),
                whitelist_duration,
            }
        }

        // === QUERIES ===
        #[ink(message)]
        pub fn config(&self) -> Config {
            Config {
                admin: self.admin,
                airdrop: self.airdrop,
                token: self.token,
                start: self.start,
                end: self.end,
                whitelist_duration: self.whitelist_duration,
            }
        }

        // === HANDLES ===
        #[ink(message)]
        pub fn whitelist_add(&mut self, address: AccountId) -> Result<AccountId> {
            let caller: AccountId = Self::env().caller();
            Self::authorise(caller, self.admin)?;

            if self.whitelist.get(address).is_some() {
                return Err(AzTokenSaleToAirdropError::UnprocessableEntity(
                    "Already on whitelist".to_string(),
                ));
            } else {
                self.whitelist.insert(address, &address);
            }

            Ok(address)
        }

        // === PRIVATE ===
        fn authorise(allowed: AccountId, received: AccountId) -> Result<()> {
            if allowed != received {
                return Err(AzTokenSaleToAirdropError::Unauthorised);
            }

            Ok(())
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use ink::env::{
            test::{default_accounts, set_caller, DefaultAccounts},
            DefaultEnvironment,
        };

        const MOCK_START: Timestamp = 654_654;
        const MOCK_END: Timestamp = 754_654;
        const MOCK_WHITELIST_DURATION: Timestamp = 1_000;

        // === HELPERS ===
        fn init() -> (DefaultAccounts<DefaultEnvironment>, AzTokenSaleToAirdrop) {
            let accounts = default_accounts();
            set_caller::<DefaultEnvironment>(accounts.bob);
            let az_token_sale_to_airdrop = AzTokenSaleToAirdrop::new(
                mock_airdrop(),
                mock_token(),
                MOCK_START,
                MOCK_END,
                MOCK_WHITELIST_DURATION,
            );
            (accounts, az_token_sale_to_airdrop)
        }

        fn mock_airdrop() -> AccountId {
            let accounts: DefaultAccounts<DefaultEnvironment> = default_accounts();
            accounts.eve
        }

        fn mock_token() -> AccountId {
            let accounts: DefaultAccounts<DefaultEnvironment> = default_accounts();
            accounts.django
        }

        // === TESTS ===
        #[ink::test]
        fn test_config() {
            let (_accounts, az_token_sale_to_airdrop) = init();
            let config = az_token_sale_to_airdrop.config();
            // * it returns the config
            assert_eq!(config.admin, az_token_sale_to_airdrop.admin);
            assert_eq!(config.airdrop, az_token_sale_to_airdrop.airdrop);
            assert_eq!(config.token, az_token_sale_to_airdrop.token);
            assert_eq!(config.start, az_token_sale_to_airdrop.start);
            assert_eq!(config.end, az_token_sale_to_airdrop.end);
            assert_eq!(
                config.whitelist_duration,
                az_token_sale_to_airdrop.whitelist_duration
            );
        }

        // === TEST HANDLES ===
        #[ink::test]
        fn test_whitelist_add() {
            let (accounts, mut az_token_sale_to_airdrop) = init();
            let new_address: AccountId = accounts.django;
            // when called by admin
            // = when address is not on whitelist
            let mut result = az_token_sale_to_airdrop.whitelist_add(new_address);
            result.unwrap();
            // = * it adds the address to whitelist
            assert_eq!(
                az_token_sale_to_airdrop
                    .whitelist
                    .get(new_address)
                    .is_some(),
                true
            );
            // = when already on whitelist
            // = * it raises an error
            result = az_token_sale_to_airdrop.whitelist_add(new_address);
            assert_eq!(
                result,
                Err(AzTokenSaleToAirdropError::UnprocessableEntity(
                    "Already on whitelist".to_string()
                ))
            );
            // when called by non admin
            // * it raises an error
            set_caller::<DefaultEnvironment>(accounts.charlie);
            result = az_token_sale_to_airdrop.whitelist_add(new_address);
            assert_eq!(result, Err(AzTokenSaleToAirdropError::Unauthorised));
        }
    }
}
