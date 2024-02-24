#![cfg_attr(not(feature = "std"), no_std, no_main)]

mod errors;

#[ink::contract]
mod az_token_sale_to_airdrop {
    use crate::errors::AzTokenSaleToAirdropError;
    use ink::{
        env::{
            call::{build_call, Call, ExecutionInput, Selector},
            CallFlags,
        },
        prelude::string::{String, ToString},
        storage::Mapping,
    };
    use primitive_types::U256;

    // === TYPES ===
    type Result<T> = core::result::Result<T, AzTokenSaleToAirdropError>;

    // === STRUCTS ===
    #[derive(scale::Decode, scale::Encode, Debug, Clone, PartialEq)]
    #[cfg_attr(
        feature = "std",
        derive(scale_info::TypeInfo, ink::storage::traits::StorageLayout)
    )]
    pub struct Buyer {
        pub total_in: Balance,
        pub whitelisted: bool,
    }

    #[derive(Debug, Clone, scale::Encode, scale::Decode)]
    #[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
    pub struct Config {
        pub admin: AccountId,
        pub airdrop_smart_contract: AccountId,
        pub in_unit: Balance,
        pub out_unit: Balance,
        pub start: Timestamp,
        pub end: Timestamp,
        pub whitelist_duration: Timestamp,
        pub in_target: Balance,
        pub in_raised: Balance,
    }

    #[derive(Debug, Clone, scale::Encode, scale::Decode)]
    #[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
    pub struct Recipient {
        pub total_amount: Balance,
        pub collected: Balance,
        // % of total_amount
        pub collectable_at_tge_percentage: u8,
        // ms from start user has to wait before either starting vesting, or collecting remaining available.
        pub cliff_duration: Timestamp,
        // ms to collect all remaining after collection at tge
        pub vesting_duration: Timestamp,
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
        ) -> Result<Self> {
            if start + whitelist_duration >= end {
                return Err(AzTokenSaleToAirdropError::UnprocessableEntity(
                    "Start + whitelist_duration must be less than end".to_string(),
                ));
            }
            if in_unit == 0 || out_unit == 0 || in_target == 0 {
                return Err(AzTokenSaleToAirdropError::UnprocessableEntity(
                    "In unit, out unit and in target must be positive".to_string(),
                ));
            }
            if in_target % in_unit > 0 {
                return Err(AzTokenSaleToAirdropError::UnprocessableEntity(
                    "In target must be a multiple of in unit".to_string(),
                ));
            }

            Ok(Self {
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
            })
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

        #[ink(message)]
        pub fn show(&self, address: AccountId) -> Buyer {
            self.buyers.get(address).unwrap_or(Buyer {
                total_in: 0,
                whitelisted: false,
            })
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
            let mut buyer: Buyer = self.show(caller);
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
            let mut in_amount: Balance = self.env().transferred_value();
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
            let max_in_amount: Balance = self.in_target - self.in_raised;
            if in_amount > max_in_amount {
                let refund_amount: Balance = in_amount - max_in_amount;
                self.transfer_azero(caller, refund_amount)?;
                in_amount = max_in_amount
            }
            let out_amount: Balance = (U256::from(in_amount) * U256::from(self.out_unit)
                / U256::from(self.in_unit))
            .as_u128();
            let description: Option<String> = None;
            // Add amount to airdrop contract
            build_call::<super::az_token_sale_to_airdrop::Environment>()
                .call_type(Call::new(self.airdrop_smart_contract))
                .exec_input(
                    ExecutionInput::new(Selector::new(ink::selector_bytes!("add_to_recipient")))
                        .push_arg(caller)
                        .push_arg(out_amount)
                        .push_arg(description),
                )
                .call_flags(CallFlags::default())
                .returns::<core::result::Result<Recipient, AzTokenSaleToAirdropError>>()
                .invoke()?;
            // Send AZERO to admin
            self.transfer_azero(self.admin, in_amount)?;
            self.in_raised += in_amount;
            buyer.total_in += in_amount;
            self.buyers.insert(caller, &buyer);

            Ok((in_amount, out_amount))
        }

        #[ink(message)]
        pub fn whitelist_add(&mut self, address: AccountId) -> Result<Buyer> {
            let caller: AccountId = Self::env().caller();
            Self::authorise(caller, self.admin)?;

            let mut buyer: Buyer = self.show(address);
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

            let mut buyer: Buyer = self.show(address);
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

        fn transfer_azero(&self, address: AccountId, amount: Balance) -> Result<()> {
            if self.env().transfer(address, amount).is_err() {
                return Err(AzTokenSaleToAirdropError::UnprocessableEntity(
                    "Insufficient AZERO balance".to_string(),
                ));
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

        const MOCK_IN_UNIT: Balance = 10;
        const MOCK_OUT_UNIT: Balance = 500;
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
            (accounts, az_token_sale_to_airdrop.expect("REASON"))
        }

        fn mock_airdrop_smart_contract() -> AccountId {
            let accounts: DefaultAccounts<DefaultEnvironment> = default_accounts();
            accounts.eve
        }

        // === TESTS ===
        // === TEST CONSTRUCTOR ===
        #[ink::test]
        fn test_new() {
            let result = AzTokenSaleToAirdrop::new(
                mock_airdrop_smart_contract(),
                MOCK_IN_UNIT,
                MOCK_OUT_UNIT,
                10,
                20,
                10,
                MOCK_IN_TARGET,
            );
            // when start + whitelist_duration is greater than or equal to end
            // * it raises an error
            assert!(result.is_err());
            // when start + whitelist_duration is less than end
            // == when in_unit is zero
            // == * it raises an error
            let result = AzTokenSaleToAirdrop::new(
                mock_airdrop_smart_contract(),
                0,
                MOCK_OUT_UNIT,
                MOCK_START,
                MOCK_END,
                MOCK_WHITELIST_DURATION,
                MOCK_IN_TARGET,
            );
            assert!(result.is_err());
            // == when in_unit is positive
            // === when out_unit is zero
            // === * it raises an error
            let result = AzTokenSaleToAirdrop::new(
                mock_airdrop_smart_contract(),
                MOCK_IN_UNIT,
                0,
                MOCK_START,
                MOCK_END,
                MOCK_WHITELIST_DURATION,
                MOCK_IN_TARGET,
            );
            assert!(result.is_err());
            // === when out_unit is positive
            // ==== when in target is zero
            let result = AzTokenSaleToAirdrop::new(
                mock_airdrop_smart_contract(),
                MOCK_IN_UNIT,
                MOCK_OUT_UNIT,
                MOCK_START,
                MOCK_END,
                MOCK_WHITELIST_DURATION,
                0,
            );
            assert!(result.is_err());
            // ==== when in target is positive
            // ===== when in target is not a multiple of in unit
            let result = AzTokenSaleToAirdrop::new(
                mock_airdrop_smart_contract(),
                MOCK_IN_UNIT,
                MOCK_OUT_UNIT,
                MOCK_START,
                MOCK_END,
                MOCK_WHITELIST_DURATION,
                MOCK_IN_TARGET + 1,
            );
            // ===== * it raises an error
            assert!(result.is_err());
            // ===== when in target is a multiple of in unit
            // ===== * it is valid
            let result = AzTokenSaleToAirdrop::new(
                mock_airdrop_smart_contract(),
                MOCK_IN_UNIT,
                MOCK_OUT_UNIT,
                MOCK_START,
                MOCK_END,
                MOCK_WHITELIST_DURATION,
                MOCK_IN_TARGET,
            );
            assert!(result.is_ok());
        }

        // === TEST QUERIES ===
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
    // The main purpose of the e2e tests are to test the interactions with az groups contract
    #[cfg(all(test, feature = "e2e-tests"))]
    mod e2e_tests {
        use super::*;
        use crate::az_token_sale_to_airdrop::AzTokenSaleToAirdropRef;
        use az_airdrop::AzAirdropRef;
        use az_button::ButtonRef;
        use ink_e2e::build_message;
        use ink_e2e::Keypair;
        use openbrush::contracts::traits::psp22::psp22_external::PSP22;

        // === CONSTANT ===
        // Token sale
        const MOCK_IN_UNIT: Balance = 1_000_000_000_000;
        const MOCK_OUT_UNIT: Balance = 50_000_000_000_000;
        const MOCK_START: Timestamp = 708_669_904_756;
        const MOCK_END: Timestamp = 2_708_669_904_756;
        const MOCK_WHITELIST_DURATION: Timestamp = 0;
        const MOCK_IN_TARGET: Balance = 50_000_000_000_000_000;

        // Airdrop
        const MOCK_AIRDROP_START: Timestamp = 2_708_669_904_756;

        // Token
        const MOCK_AMOUNT: Balance = 100_000_000_000_000_000_000;

        // === TYPES ===
        type E2EResult<T> = std::result::Result<T, Box<dyn std::error::Error>>;

        // === HELPERS ===
        fn account_id(k: Keypair) -> AccountId {
            AccountId::try_from(k.public_key().to_account_id().as_ref())
                .expect("account keyring has a valid account id")
        }

        // === TEST HANDLES ===
        #[ink_e2e::test]
        async fn test_buy(mut client: ::ink_e2e::Client<C, E>) -> E2EResult<()> {
            let alice_account_id: AccountId = account_id(ink_e2e::alice());
            let bob_account_id: AccountId = account_id(ink_e2e::bob());

            // Instantiate token
            let token_constructor = ButtonRef::new(
                MOCK_AMOUNT,
                Some("DIBS".to_string()),
                Some("DIBS".to_string()),
                12,
            );
            let token_id: AccountId = client
                .instantiate("az_button", &ink_e2e::alice(), token_constructor, 0, None)
                .await
                .expect("Token instantiate failed")
                .account_id;

            // Instantiate airdrop smart contract
            let default_collectable_at_tge_percentage: u8 = 20;
            let default_cliff_duration: Timestamp = 0;
            let default_vesting_duration: Timestamp = 31_556_952_000;
            let airdrop_constructor = AzAirdropRef::new(
                token_id,
                MOCK_AIRDROP_START,
                default_collectable_at_tge_percentage,
                default_cliff_duration,
                default_vesting_duration,
            );
            let airdrop_id: AccountId = client
                .instantiate(
                    "az_airdrop",
                    &ink_e2e::alice(),
                    airdrop_constructor,
                    0,
                    None,
                )
                .await
                .expect("Airdrop instantiate failed")
                .account_id;
            // send tokens to airdrop smart contract
            let transfer_message = build_message::<ButtonRef>(token_id)
                .call(|button| button.transfer(airdrop_id, MOCK_AMOUNT, vec![]));
            let transfer_result = client
                .call(&ink_e2e::alice(), transfer_message, 0, None)
                .await
                .unwrap()
                .dry_run
                .exec_result
                .result;
            assert!(transfer_result.is_ok());

            // Instantiate token sale smart contract
            let token_sale_contractor = AzTokenSaleToAirdropRef::new(
                airdrop_id,
                MOCK_IN_UNIT,
                MOCK_OUT_UNIT,
                MOCK_START,
                MOCK_END,
                MOCK_WHITELIST_DURATION,
                MOCK_IN_TARGET,
            );
            let token_sale_id: AccountId = client
                .instantiate(
                    "az_token_sale_to_airdrop",
                    &ink_e2e::alice(),
                    token_sale_contractor,
                    0,
                    None,
                )
                .await
                .expect("Token sale instantiate failed")
                .account_id;
            // add token_sale_id as sub-admin of airdrop smart contract
            let sub_admins_add_message = build_message::<AzAirdropRef>(airdrop_id)
                .call(|airdrop| airdrop.sub_admins_add(token_sale_id));
            let sub_admins_add_result = client
                .call(&ink_e2e::alice(), sub_admins_add_message, 0, None)
                .await
                .unwrap()
                .dry_run
                .exec_result
                .result;
            assert!(sub_admins_add_result.is_ok());

            // when sale has started
            // = when in public phase
            // == when in amount is positive
            // === when in amount is a multiple of in_unit
            // ==== when there is enough stock to fill full order
            let original_alice_azero_balance: Balance =
                client.balance(alice_account_id).await.unwrap();
            let original_bob_azero_balance: Balance = client.balance(bob_account_id).await.unwrap();
            let buy_message = build_message::<AzTokenSaleToAirdropRef>(token_sale_id)
                .call(|token_sale| token_sale.buy());
            let buy_result = client
                .call(&ink_e2e::bob(), buy_message, MOCK_IN_UNIT, None)
                .await
                .unwrap()
                .dry_run
                .exec_result
                .result;
            assert!(buy_result.is_ok());
            // ==== * it increases the recipient amount on airdrop by the out amount
            let airdrop_show_message = build_message::<AzAirdropRef>(airdrop_id)
                .call(|airdrop| airdrop.show(bob_account_id));
            let result = client
                .call_dry_run(&ink_e2e::alice(), &airdrop_show_message, 0, None)
                .await
                .return_value();
            assert_eq!(result.unwrap().total_amount, MOCK_OUT_UNIT);
            // ==== * it increases the in_raised by the in amount
            let config_message = build_message::<AzTokenSaleToAirdropRef>(token_sale_id)
                .call(|token_sale| token_sale.config());
            let result = client
                .call_dry_run(&ink_e2e::alice(), &config_message, 0, None)
                .await
                .return_value();
            assert_eq!(result.in_raised, MOCK_IN_UNIT);
            // ==== * it increases the buyers total_in amount
            let buyer_show_message = build_message::<AzTokenSaleToAirdropRef>(token_sale_id)
                .call(|token_sale| token_sale.show(bob_account_id));
            let result = client
                .call_dry_run(&ink_e2e::alice(), &buyer_show_message, 0, None)
                .await
                .return_value();
            assert_eq!(result.total_in, MOCK_IN_UNIT);
            // ==== * it sends the in_amount to the admin
            assert_eq!(
                client.balance(alice_account_id).await.unwrap(),
                original_alice_azero_balance + MOCK_IN_UNIT
            );
            // ==== when there is only enough stock to partially fill order
            let buy_message = build_message::<AzTokenSaleToAirdropRef>(token_sale_id)
                .call(|token_sale| token_sale.buy());
            let buy_result = client
                .call(&ink_e2e::bob(), buy_message, MOCK_IN_TARGET, None)
                .await
                .unwrap()
                .dry_run
                .exec_result
                .result;
            assert!(buy_result.is_ok());
            // ==== * it increases the recipient amount on airdrop by the available out amount
            let airdrop_show_message = build_message::<AzAirdropRef>(airdrop_id)
                .call(|airdrop| airdrop.show(bob_account_id));
            let result = client
                .call_dry_run(&ink_e2e::alice(), &airdrop_show_message, 0, None)
                .await
                .return_value();
            assert_eq!(
                result.unwrap().total_amount,
                MOCK_OUT_UNIT * MOCK_IN_TARGET / MOCK_IN_UNIT
            );
            // ==== * it increases the in_raised by the avaiable in amount
            let config_message = build_message::<AzTokenSaleToAirdropRef>(token_sale_id)
                .call(|token_sale| token_sale.config());
            let result = client
                .call_dry_run(&ink_e2e::alice(), &config_message, 0, None)
                .await
                .return_value();
            assert_eq!(result.in_raised, MOCK_IN_TARGET);
            // ==== * it increases the buyers total_in by the available amount
            let buyer_show_message = build_message::<AzTokenSaleToAirdropRef>(token_sale_id)
                .call(|token_sale| token_sale.show(bob_account_id));
            let result = client
                .call_dry_run(&ink_e2e::alice(), &buyer_show_message, 0, None)
                .await
                .return_value();
            assert_eq!(result.total_in, MOCK_IN_TARGET);
            // ==== * it sends the in_amount to the admin
            assert_eq!(
                client.balance(alice_account_id).await.unwrap(),
                original_alice_azero_balance + MOCK_IN_TARGET
            );
            // ==== * it refunds the unused in_amount
            assert!(
                client.balance(bob_account_id).await.unwrap()
                    > original_bob_azero_balance - MOCK_IN_TARGET - MOCK_IN_UNIT
            );

            Ok(())
        }
    }
}
