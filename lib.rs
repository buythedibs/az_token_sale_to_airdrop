#![cfg_attr(not(feature = "std"), no_std, no_main)]

#[ink::contract]
mod az_token_sale_to_airdrop {
    // === STRUCTS ===
    #[derive(Debug, Clone, scale::Encode, scale::Decode)]
    #[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
    pub struct Config {
        airdrop: AccountId,
        token: AccountId,
    }

    // === CONTRACT ===
    #[ink(storage)]
    pub struct AzTokenSaleToAirdrop {
        airdrop: AccountId,
        token: AccountId,
    }
    impl AzTokenSaleToAirdrop {
        #[ink(constructor)]
        pub fn new(airdrop: AccountId, token: AccountId) -> Self {
            Self { airdrop, token }
        }

        // === QUERIES ===
        #[ink(message)]
        pub fn config(&self) -> Config {
            Config {
                airdrop: self.airdrop,
                token: self.token,
            }
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use ink::env::{
            test::{default_accounts, set_caller, DefaultAccounts},
            DefaultEnvironment,
        };

        // === HELPERS ===
        fn init() -> (DefaultAccounts<DefaultEnvironment>, AzTokenSaleToAirdrop) {
            let accounts = default_accounts();
            set_caller::<DefaultEnvironment>(accounts.bob);
            let az_token_sale_to_airdrop = AzTokenSaleToAirdrop::new(mock_airdrop(), mock_token());
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
            assert_eq!(config.airdrop, az_token_sale_to_airdrop.airdrop);
            assert_eq!(config.token, az_token_sale_to_airdrop.token);
        }
    }
}
