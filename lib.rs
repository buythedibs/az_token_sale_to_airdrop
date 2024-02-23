#![cfg_attr(not(feature = "std"), no_std, no_main)]

mod errors;

#[ink::contract]
mod az_token_sale_to_airdrop {
    use crate::errors::AzTokenSaleToAirdropError;
    use ink::{prelude::string::ToString, storage::Mapping};

    // === TYPES ===
    type Result<T> = core::result::Result<T, AzTokenSaleToAirdropError>;

    // === STRUCTS ===
    #[derive(scale::Decode, scale::Encode, Debug, Clone, PartialEq)]
    #[cfg_attr(
        feature = "std",
        derive(scale_info::TypeInfo, ink::storage::traits::StorageLayout)
    )]
    pub struct Buyer {
        total_in: Balance,
        whitelisted: bool,
    }

    #[derive(Debug, Clone, scale::Encode, scale::Decode)]
    #[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
    pub struct Config {
        admin: AccountId,
        airdrop_smart_contract: AccountId,
        in_unit: Balance,
        out_unit: Balance,
        start: Timestamp,
        end: Timestamp,
        whitelist_duration: Timestamp,
        in_target: Balance,
        in_raised: Balance,
    }

    // === CONTRACT ===
    #[ink(storage)]
    pub struct AzTokenSaleToAirdrop {
        admin: AccountId,
        airdrop_smart_contract: AccountId,
        in_unit: Balance,
        out_unit: Balance,
        start: Timestamp,
        end: Timestamp,
        buyers: Mapping<AccountId, Buyer>,
        whitelist_duration: Timestamp,
        in_target: Balance,
        in_raised: Balance,
    }
    impl AzTokenSaleToAirdrop {
        #[ink(constructor)]
        pub fn new(
            airdrop_smart_contract: AccountId,
            in_unit: Balance,
            out_unit: Balance,
            start: Timestamp,
            end: Timestamp,
            whitelist_duration: Timestamp,
            in_target: Balance,
        ) -> Self {
            Self {
                admin: Self::env().caller(),
                airdrop_smart_contract,
                in_unit,
                out_unit,
                start,
                end,
                buyers: Mapping::default(),
                whitelist_duration,
                in_target,
                in_raised: 0,
            }
        }

        // === QUERIES ===
        #[ink(message)]
        pub fn config(&self) -> Config {
            Config {
                admin: self.admin,
                airdrop_smart_contract: self.airdrop_smart_contract,
                in_unit: self.in_unit,
                out_unit: self.out_unit,
                start: self.start,
                end: self.end,
                whitelist_duration: self.whitelist_duration,
                in_target: self.in_target,
                in_raised: self.in_raised,
            }
        }

        // === HANDLES ===
        #[ink(message, payable)]
        pub fn buy(&mut self) -> Result<(Balance, Balance)> {
            let block_timestamp: Timestamp = Self::env().block_timestamp();
            // validate sale has started
            if block_timestamp < self.start {
                return Err(AzTokenSaleToAirdropError::UnprocessableEntity(
                    "Sale has not started".to_string(),
                ));
            }
            // validate sale has not ended
            if block_timestamp > self.end {
                return Err(AzTokenSaleToAirdropError::UnprocessableEntity(
                    "Sale has ended".to_string(),
                ));
            }
            // validate user is on whitelist if during whitelist duration
            let caller: AccountId = Self::env().caller();
            let mut buyer: Buyer = self.buyers.get(caller).unwrap_or(Buyer {
                total_in: 0,
                whitelisted: false,
            });
            if self.whitelist_duration > 0
                && block_timestamp < (self.start + self.whitelist_duration)
            {
                if !buyer.whitelisted {
                    return Err(AzTokenSaleToAirdropError::UnprocessableEntity(
                        "Sale is currently only available to whitelisted addresses".to_string(),
                    ));
                }
            }
            // validate in amount is in units of in_unit
            let in_amount: Balance = self.env().transferred_value();
            if in_amount == 0 || in_amount % self.in_unit > 0 {
                return Err(AzTokenSaleToAirdropError::UnprocessableEntity(
                    "In amount must be in multiples of in_unit".to_string(),
                ));
            }
            // validate sold out
            if self.in_raised == self.in_target {
                return Err(AzTokenSaleToAirdropError::UnprocessableEntity(
                    "Sold out".to_string(),
                ));
            }
            let out_amount: Balance = in_amount * self.out_unit / self.in_unit;

            Ok((in_amount, out_amount))
        }

        #[ink(message)]
        pub fn whitelist_add(&mut self, address: AccountId) -> Result<Buyer> {
            let caller: AccountId = Self::env().caller();
            Self::authorise(caller, self.admin)?;

            let mut buyer: Buyer = self.buyers.get(address).unwrap_or(Buyer {
                total_in: 0,
                whitelisted: false,
            });
            if buyer.whitelisted {
                return Err(AzTokenSaleToAirdropError::UnprocessableEntity(
                    "Already on whitelist".to_string(),
                ));
            } else {
                buyer.whitelisted = true;
                self.buyers.insert(address, &buyer);
            }

            Ok(buyer)
        }

        #[ink(message)]
        pub fn whitelist_remove(&mut self, address: AccountId) -> Result<Buyer> {
            let caller: AccountId = Self::env().caller();
            Self::authorise(caller, self.admin)?;

            let mut buyer: Buyer = self.buyers.get(address).unwrap_or(Buyer {
                total_in: 0,
                whitelisted: false,
            });
            if buyer.whitelisted {
                buyer.whitelisted = false;
                self.buyers.insert(address, &buyer);
            } else {
                return Err(AzTokenSaleToAirdropError::UnprocessableEntity(
                    "Not on whitelist".to_string(),
                ));
            }

            Ok(buyer)
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

        const MOCK_IN_UNIT: Balance = 654_654;
        const MOCK_OUT_UNIT: Balance = 654_654;
        const MOCK_START: Timestamp = 654_654;
        const MOCK_END: Timestamp = 754_654;
        const MOCK_WHITELIST_DURATION: Timestamp = 1_000;
        const MOCK_IN_TARGET: Balance = 50_000_000_000_000_000;

        // === HELPERS ===
        fn init() -> (DefaultAccounts<DefaultEnvironment>, AzTokenSaleToAirdrop) {
            let accounts = default_accounts();
            set_caller::<DefaultEnvironment>(accounts.bob);
            let az_token_sale_to_airdrop = AzTokenSaleToAirdrop::new(
                mock_airdrop_smart_contract(),
                MOCK_IN_UNIT,
                MOCK_OUT_UNIT,
                MOCK_START,
                MOCK_END,
                MOCK_WHITELIST_DURATION,
                MOCK_IN_TARGET,
            );
            (accounts, az_token_sale_to_airdrop)
        }

        fn mock_airdrop_smart_contract() -> AccountId {
            let accounts: DefaultAccounts<DefaultEnvironment> = default_accounts();
            accounts.eve
        }

        // === TESTS ===
        #[ink::test]
        fn test_config() {
            let (_accounts, az_token_sale_to_airdrop) = init();
            let config = az_token_sale_to_airdrop.config();
            // * it returns the config
            assert_eq!(config.admin, az_token_sale_to_airdrop.admin);
            assert_eq!(
                config.airdrop_smart_contract,
                az_token_sale_to_airdrop.airdrop_smart_contract
            );
            assert_eq!(config.in_unit, az_token_sale_to_airdrop.in_unit);
            assert_eq!(config.out_unit, az_token_sale_to_airdrop.out_unit);
            assert_eq!(config.start, az_token_sale_to_airdrop.start);
            assert_eq!(config.end, az_token_sale_to_airdrop.end);
            assert_eq!(
                config.whitelist_duration,
                az_token_sale_to_airdrop.whitelist_duration
            );
            assert_eq!(config.in_target, az_token_sale_to_airdrop.in_target);
        }

        // === TEST HANDLES ===
        #[ink::test]
        fn test_buy() {
            let (accounts, mut az_token_sale_to_airdrop) = init();
            // when sale has not started
            ink::env::test::set_block_timestamp::<ink::env::DefaultEnvironment>(
                az_token_sale_to_airdrop.start - 1,
            );
            // * it raises an error
            let mut result = az_token_sale_to_airdrop.buy();
            assert_eq!(
                result,
                Err(AzTokenSaleToAirdropError::UnprocessableEntity(
                    "Sale has not started".to_string()
                ))
            );
            // when sale has started
            // = when sale has ended
            ink::env::test::set_block_timestamp::<ink::env::DefaultEnvironment>(
                az_token_sale_to_airdrop.end + 1,
            );
            // = * it raises an error
            result = az_token_sale_to_airdrop.buy();
            assert_eq!(
                result,
                Err(AzTokenSaleToAirdropError::UnprocessableEntity(
                    "Sale has ended".to_string()
                ))
            );
            // == when in whitelist phase
            ink::env::test::set_block_timestamp::<ink::env::DefaultEnvironment>(
                az_token_sale_to_airdrop.start + az_token_sale_to_airdrop.whitelist_duration - 1,
            );
            // === when buyer is not on whitelist
            // === * it raises an error
            result = az_token_sale_to_airdrop.buy();
            assert_eq!(
                result,
                Err(AzTokenSaleToAirdropError::UnprocessableEntity(
                    "Sale is currently only available to whitelisted addresses".to_string()
                ))
            );
            // === when buyer is on whitelist
            az_token_sale_to_airdrop.buyers.insert(
                accounts.bob,
                &Buyer {
                    total_in: 0,
                    whitelisted: true,
                },
            );
            // ==== when in amount is zero
            // ==== * it raises an error
            result = az_token_sale_to_airdrop.buy();
            assert_eq!(
                result,
                Err(AzTokenSaleToAirdropError::UnprocessableEntity(
                    "In amount must be in multiples of in_unit".to_string()
                ))
            );
            // ==== when in amount is positive
            // ===== when in amount is not a multiple of in_unit
            // ===== * it raises an error
            ink::env::test::set_value_transferred::<ink::env::DefaultEnvironment>(MOCK_IN_UNIT + 1);
            result = az_token_sale_to_airdrop.buy();
            assert_eq!(
                result,
                Err(AzTokenSaleToAirdropError::UnprocessableEntity(
                    "In amount must be in multiples of in_unit".to_string()
                ))
            );
            ink::env::test::set_value_transferred::<ink::env::DefaultEnvironment>(MOCK_IN_UNIT - 1);
            result = az_token_sale_to_airdrop.buy();
            assert_eq!(
                result,
                Err(AzTokenSaleToAirdropError::UnprocessableEntity(
                    "In amount must be in multiples of in_unit".to_string()
                ))
            );
            // ===== when in amount is a multiple of in_unit
            ink::env::test::set_value_transferred::<ink::env::DefaultEnvironment>(MOCK_IN_UNIT);
            // ====== when there is no more available for sale
            az_token_sale_to_airdrop.in_raised = az_token_sale_to_airdrop.in_target;
            // ====== * it raises an error
            result = az_token_sale_to_airdrop.buy();
            assert_eq!(
                result,
                Err(AzTokenSaleToAirdropError::UnprocessableEntity(
                    "Sold out".to_string()
                ))
            );
            // ====== when there is stock available
            // REST WILL HAVE TO GO INTO INTEGRATION TEST AS IT CALLS AIRDROP SMART CONTRACT
        }

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
                    .buyers
                    .get(new_address)
                    .unwrap()
                    .whitelisted,
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

        #[ink::test]
        fn test_whitelist_remove() {
            let (accounts, mut az_token_sale_to_airdrop) = init();
            let address_to_remove: AccountId = accounts.django;
            // when called by admin
            // = when not on whitelist
            let mut result = az_token_sale_to_airdrop.whitelist_remove(address_to_remove);
            assert_eq!(
                result,
                Err(AzTokenSaleToAirdropError::UnprocessableEntity(
                    "Not on whitelist".to_string()
                ))
            );
            // = when on whitelist
            az_token_sale_to_airdrop
                .whitelist_add(address_to_remove)
                .unwrap();
            result = az_token_sale_to_airdrop.whitelist_remove(address_to_remove);
            result.unwrap();
            // = * it remove the address from whitelist
            assert_eq!(
                az_token_sale_to_airdrop
                    .buyers
                    .get(address_to_remove)
                    .unwrap()
                    .whitelisted,
                false
            );
            // when called by non admin
            // * it raises an error
            set_caller::<DefaultEnvironment>(accounts.charlie);
            result = az_token_sale_to_airdrop.whitelist_remove(address_to_remove);
            assert_eq!(result, Err(AzTokenSaleToAirdropError::Unauthorised));
        }
    }
}
