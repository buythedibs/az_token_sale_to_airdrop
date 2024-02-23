#![cfg_attr(not(feature = "std"), no_std, no_main)]

pub use self::az_airdrop::AzAirdropRef;

mod errors;

#[ink::contract]
mod az_airdrop {
    use crate::errors::AzAirdropError;
    use ink::{
        codegen::EmitEvent,
        env::CallFlags,
        prelude::string::{String, ToString},
        prelude::{vec, vec::Vec},
        reflect::ContractEventBase,
        storage::{Lazy, Mapping},
    };
    use openbrush::contracts::psp22::PSP22Ref;
    use primitive_types::U256;

    // === TYPES ===
    type Event = <AzAirdrop as ContractEventBase>::Type;
    type Result<T> = core::result::Result<T, AzAirdropError>;

    // === EVENTS ===
    #[ink(event)]
    pub struct AddToRecipient {
        #[ink(topic)]
        address: AccountId,
        amount: Balance,
        description: Option<String>,
    }

    #[ink(event)]
    pub struct SubtractFromRecipient {
        #[ink(topic)]
        address: AccountId,
        amount: Balance,
        description: Option<String>,
    }

    // === STRUCTS ===
    #[derive(Debug, Clone, scale::Encode, scale::Decode)]
    #[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
    pub struct Config {
        admin: AccountId,
        sub_admins: Vec<AccountId>,
        token: AccountId,
        to_be_collected: Balance,
        start: Timestamp,
        default_collectable_at_tge_percentage: u8,
        default_cliff_duration: Timestamp,
        default_vesting_duration: Timestamp,
    }

    #[derive(scale::Decode, scale::Encode, Debug, Clone, PartialEq)]
    #[cfg_attr(
        feature = "std",
        derive(scale_info::TypeInfo, ink::storage::traits::StorageLayout)
    )]
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
    pub struct AzAirdrop {
        admin: AccountId,
        sub_admins_mapping: Mapping<AccountId, AccountId>,
        sub_admins_as_vec: Lazy<Vec<AccountId>>,
        token: AccountId,
        to_be_collected: Balance,
        start: Timestamp,
        recipients: Mapping<AccountId, Recipient>,
        default_collectable_at_tge_percentage: u8,
        default_cliff_duration: Timestamp,
        default_vesting_duration: Timestamp,
    }
    impl AzAirdrop {
        #[ink(constructor)]
        pub fn new(
            token: AccountId,
            start: Timestamp,
            default_collectable_at_tge_percentage: u8,
            default_cliff_duration: Timestamp,
            default_vesting_duration: Timestamp,
        ) -> Result<Self> {
            Self::validate_airdrop_calculation_variables(
                start,
                default_collectable_at_tge_percentage,
                default_cliff_duration,
                default_vesting_duration,
            )?;

            Ok(Self {
                admin: Self::env().caller(),
                sub_admins_mapping: Mapping::default(),
                sub_admins_as_vec: Default::default(),
                token,
                to_be_collected: 0,
                start,
                recipients: Mapping::default(),
                default_collectable_at_tge_percentage,
                default_cliff_duration,
                default_vesting_duration,
            })
        }

        // === QUERIES ===
        // 0 = start (collectable_at_tge)
        // 1 = vesting_start = start + cliff_duration
        // 2 = vesting_end = vesting_start + vesting_duration
        #[ink(message)]
        pub fn collectable_amount(
            &self,
            address: AccountId,
            timestamp: Timestamp,
        ) -> Result<Balance> {
            let recipient: Recipient = self.show(address)?;
            let mut total_collectable_at_time: Balance = 0;
            if timestamp >= self.start {
                // collectable at tge
                let collectable_at_tge: Balance =
                    (U256::from(recipient.collectable_at_tge_percentage)
                        * U256::from(recipient.total_amount)
                        / U256::from(100))
                    .as_u128();
                total_collectable_at_time = collectable_at_tge;
                if recipient.vesting_duration > 0 {
                    // This can't overflow as checks are done in validate_airdrop_calculation_variables
                    let vesting_start: Timestamp = self.start + recipient.cliff_duration;
                    let mut vesting_collectable: Balance = 0;
                    if timestamp >= vesting_start {
                        // This can't overflow
                        let vesting_time_reached: Timestamp = timestamp - vesting_start;
                        // This can't overflow
                        let collectable_during_vesting: Balance =
                            recipient.total_amount - collectable_at_tge;
                        vesting_collectable = (U256::from(vesting_time_reached)
                            * U256::from(collectable_during_vesting)
                            / U256::from(recipient.vesting_duration))
                        .as_u128();
                    }
                    // This can't overflow
                    total_collectable_at_time = total_collectable_at_time + vesting_collectable;
                }
                if total_collectable_at_time > recipient.total_amount {
                    total_collectable_at_time = recipient.total_amount
                }
            }

            Ok(total_collectable_at_time.saturating_sub(recipient.collected))
        }

        #[ink(message)]
        pub fn config(&self) -> Config {
            Config {
                admin: self.admin,
                sub_admins: self.sub_admins_as_vec.get_or_default(),
                token: self.token,
                to_be_collected: self.to_be_collected,
                start: self.start,
                default_collectable_at_tge_percentage: self.default_collectable_at_tge_percentage,
                default_cliff_duration: self.default_cliff_duration,
                default_vesting_duration: self.default_vesting_duration,
            }
        }

        #[ink(message)]
        pub fn show(&self, address: AccountId) -> Result<Recipient> {
            self.recipients
                .get(address)
                .ok_or(AzAirdropError::NotFound("Recipient".to_string()))
        }

        // === HANDLES ===
        // Not a must, but good to have function
        #[ink(message)]
        pub fn acquire_token(&mut self, amount: Balance, from: AccountId) -> Result<()> {
            let caller: AccountId = Self::env().caller();
            Self::authorise(caller, self.admin)?;
            self.airdrop_has_not_started()?;

            PSP22Ref::transfer_from_builder(
                &self.token,
                from,
                self.env().account_id(),
                amount,
                vec![],
            )
            .call_flags(CallFlags::default())
            .invoke()?;

            Ok(())
        }

        // This is for the sales smart contract to call
        #[ink(message)]
        pub fn add_to_recipient(
            &mut self,
            address: AccountId,
            amount: Balance,
            description: Option<String>,
        ) -> Result<Recipient> {
            self.authorise_to_update_recipient()?;
            self.airdrop_has_not_started()?;
            if let Some(new_to_be_collected) = amount.checked_add(self.to_be_collected) {
                // Check that balance has enough to cover
                let smart_contract_balance: Balance =
                    PSP22Ref::balance_of(&self.token, Self::env().account_id());
                if new_to_be_collected > smart_contract_balance {
                    return Err(AzAirdropError::UnprocessableEntity(
                        "Insufficient balance".to_string(),
                    ));
                }

                let mut recipient: Recipient = self.recipients.get(address).unwrap_or(Recipient {
                    total_amount: 0,
                    collected: 0,
                    collectable_at_tge_percentage: self.default_collectable_at_tge_percentage,
                    cliff_duration: self.default_cliff_duration,
                    vesting_duration: self.default_vesting_duration,
                });
                // This can't overflow
                recipient.total_amount += amount;
                self.recipients.insert(address, &recipient);
                self.to_be_collected = new_to_be_collected;

                // emit event
                Self::emit_event(
                    self.env(),
                    Event::AddToRecipient(AddToRecipient {
                        address,
                        amount,
                        description,
                    }),
                );

                Ok(recipient)
            } else {
                return Err(AzAirdropError::UnprocessableEntity(
                    "Amount will cause to_be_collected to overflow".to_string(),
                ));
            }
        }

        #[ink(message)]
        pub fn collect(&mut self) -> Result<Balance> {
            let caller: AccountId = Self::env().caller();
            let mut recipient = self.show(caller)?;

            let block_timestamp: Timestamp = Self::env().block_timestamp();
            let collectable_amount: Balance = self.collectable_amount(caller, block_timestamp)?;
            if collectable_amount == 0 {
                return Err(AzAirdropError::UnprocessableEntity(
                    "Amount is zero".to_string(),
                ));
            }

            // transfer to caller
            PSP22Ref::transfer_builder(&self.token, caller, collectable_amount, vec![])
                .call_flags(CallFlags::default())
                .invoke()?;
            // increase recipient's collected
            // These can't overflow, but might as well
            recipient.collected = recipient.collected.saturating_add(collectable_amount);
            self.recipients.insert(caller, &recipient);
            self.to_be_collected = self.to_be_collected.saturating_sub(collectable_amount);

            Ok(collectable_amount)
        }

        #[ink(message)]
        pub fn return_spare_tokens(&mut self) -> Result<Balance> {
            let caller: AccountId = Self::env().caller();
            let contract_address: AccountId = Self::env().account_id();
            Self::authorise(caller, self.admin)?;

            let balance: Balance = PSP22Ref::balance_of(&self.token, contract_address);
            // These can't overflow, but might as well
            let spare_amount: Balance = balance.saturating_sub(self.to_be_collected);
            if spare_amount > 0 {
                PSP22Ref::transfer_builder(&self.token, caller, spare_amount, vec![])
                    .call_flags(CallFlags::default())
                    .invoke()?;
            } else {
                return Err(AzAirdropError::UnprocessableEntity(
                    "Amount is zero".to_string(),
                ));
            }

            Ok(spare_amount)
        }

        #[ink(message)]
        pub fn subtract_from_recipient(
            &mut self,
            address: AccountId,
            amount: Balance,
            description: Option<String>,
        ) -> Result<Recipient> {
            self.authorise_to_update_recipient()?;
            self.airdrop_has_not_started()?;
            let mut recipient = self.show(address)?;
            if amount > recipient.total_amount {
                return Err(AzAirdropError::UnprocessableEntity(
                    "Amount is greater than recipient's total amount".to_string(),
                ));
            }

            // Update recipient
            // This can't overflow because of the above check
            recipient.total_amount -= amount;
            self.recipients.insert(address, &recipient);

            // Update config
            // This can't overflow but might as well
            self.to_be_collected = self.to_be_collected.saturating_sub(amount);

            // emit event
            Self::emit_event(
                self.env(),
                Event::SubtractFromRecipient(SubtractFromRecipient {
                    address,
                    amount,
                    description,
                }),
            );

            Ok(recipient)
        }

        #[ink(message)]
        pub fn sub_admins_add(&mut self, address: AccountId) -> Result<Vec<AccountId>> {
            let caller: AccountId = Self::env().caller();
            Self::authorise(caller, self.admin)?;

            let mut sub_admins: Vec<AccountId> = self.sub_admins_as_vec.get_or_default();
            if self.sub_admins_mapping.get(address).is_some() {
                return Err(AzAirdropError::UnprocessableEntity(
                    "Already a sub admin".to_string(),
                ));
            } else {
                sub_admins.push(address.clone());
                self.sub_admins_mapping.insert(address, &address.clone());
            }
            self.sub_admins_as_vec.set(&sub_admins);

            Ok(sub_admins)
        }

        #[ink(message)]
        pub fn sub_admins_remove(&mut self, address: AccountId) -> Result<Vec<AccountId>> {
            let caller: AccountId = Self::env().caller();
            Self::authorise(caller, self.admin)?;

            let mut sub_admins: Vec<AccountId> = self.sub_admins_as_vec.get_or_default();
            if self.sub_admins_mapping.get(address).is_none() {
                return Err(AzAirdropError::UnprocessableEntity(
                    "Not a sub admin".to_string(),
                ));
            } else {
                let index = sub_admins.iter().position(|x| *x == address).unwrap();
                sub_admins.remove(index);
                self.sub_admins_mapping.remove(address);
            }
            self.sub_admins_as_vec.set(&sub_admins);

            Ok(sub_admins)
        }

        // #[derive(Debug, Clone, scale::Encode, scale::Decode)]
        // #[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
        // pub struct Config {
        //     admin: AccountId,
        //     sub_admins: Vec<AccountId>,
        //     token: AccountId,
        //     to_be_collected: Balance,
        //     start: Timestamp,
        //     default_collectable_at_tge_percentage: u8,
        //     default_cliff_duration: Timestamp,
        //     default_vesting_duration: Timestamp,
        // }
        #[ink(message)]
        pub fn update_config(
            &mut self,
            admin: Option<AccountId>,
            start: Option<Timestamp>,
            default_collectable_at_tge_percentage: Option<u8>,
            default_cliff_duration: Option<Timestamp>,
            default_vesting_duration: Option<Timestamp>,
        ) -> Result<()> {
            let caller: AccountId = Self::env().caller();
            Self::authorise(caller, self.admin)?;

            if let Some(admin_unwrapped) = admin {
                self.admin = admin_unwrapped
            }
            if let Some(start_unwrapped) = start {
                let block_timestamp: Timestamp = Self::env().block_timestamp();
                if start_unwrapped > block_timestamp {
                    if self.to_be_collected == 0 {
                        self.start = start_unwrapped
                    } else {
                        return Err(AzAirdropError::UnprocessableEntity(
                            "to_be_collected must be zero when changing start time".to_string(),
                        ));
                    }
                } else {
                    return Err(AzAirdropError::UnprocessableEntity(
                        "New start time must be in the future".to_string(),
                    ));
                }
            }
            if let Some(default_collectable_at_tge_percentage_unwrapped) =
                default_collectable_at_tge_percentage
            {
                self.default_collectable_at_tge_percentage =
                    default_collectable_at_tge_percentage_unwrapped
            }
            if let Some(default_cliff_duration_unwrapped) = default_cliff_duration {
                self.default_cliff_duration = default_cliff_duration_unwrapped
            }
            if let Some(default_vesting_duration_unwrapped) = default_vesting_duration {
                self.default_vesting_duration = default_vesting_duration_unwrapped
            }
            Self::validate_airdrop_calculation_variables(
                self.start,
                self.default_collectable_at_tge_percentage,
                self.default_cliff_duration,
                self.default_vesting_duration,
            )?;

            // Will not let me check exact error
            // when Config is returned
            Ok(())
        }

        #[ink(message)]
        pub fn update_recipient(
            &mut self,
            address: AccountId,
            collectable_at_tge_percentage: Option<u8>,
            cliff_duration: Option<Timestamp>,
            vesting_duration: Option<Timestamp>,
        ) -> Result<Recipient> {
            self.authorise_to_update_recipient()?;
            self.airdrop_has_not_started()?;
            let mut recipient: Recipient = self.show(address)?;

            if let Some(collectable_at_tge_percentage_unwrapped) = collectable_at_tge_percentage {
                recipient.collectable_at_tge_percentage = collectable_at_tge_percentage_unwrapped
            }
            if let Some(cliff_duration_unwrapped) = cliff_duration {
                recipient.cliff_duration = cliff_duration_unwrapped
            }
            if let Some(vesting_duration_unwrapped) = vesting_duration {
                recipient.vesting_duration = vesting_duration_unwrapped
            }
            Self::validate_airdrop_calculation_variables(
                self.start,
                recipient.collectable_at_tge_percentage,
                recipient.cliff_duration,
                recipient.vesting_duration,
            )?;

            self.recipients.insert(address, &recipient);

            Ok(recipient)
        }

        // === PRIVATE ===
        fn airdrop_has_not_started(&self) -> Result<()> {
            let block_timestamp: Timestamp = Self::env().block_timestamp();
            if block_timestamp >= self.start {
                return Err(AzAirdropError::UnprocessableEntity(
                    "Airdrop has started".to_string(),
                ));
            }

            Ok(())
        }

        fn authorise(allowed: AccountId, received: AccountId) -> Result<()> {
            if allowed != received {
                return Err(AzAirdropError::Unauthorised);
            }

            Ok(())
        }

        fn authorise_to_update_recipient(&self) -> Result<()> {
            let caller: AccountId = Self::env().caller();
            if caller == self.admin || self.sub_admins_mapping.get(caller).is_some() {
                Ok(())
            } else {
                return Err(AzAirdropError::Unauthorised);
            }
        }

        fn emit_event<EE: EmitEvent<Self>>(emitter: EE, event: Event) {
            emitter.emit_event(event);
        }

        fn validate_airdrop_calculation_variables(
            start: Timestamp,
            collectable_at_tge_percentage: u8,
            cliff_duration: Timestamp,
            vesting_duration: Timestamp,
        ) -> Result<()> {
            if collectable_at_tge_percentage > 100 {
                return Err(AzAirdropError::UnprocessableEntity(
                    "collectable_at_tge_percentage must be less than or equal to 100".to_string(),
                ));
            } else if collectable_at_tge_percentage == 100 {
                if cliff_duration > 0 || vesting_duration > 0 {
                    return Err(AzAirdropError::UnprocessableEntity(
                        "cliff_duration and vesting_duration must be 0 when collectable_tge_percentage is 100"
                            .to_string(),
                    ));
                }
            } else if vesting_duration == 0 {
                return Err(AzAirdropError::UnprocessableEntity(
                    "vesting_duration must be greater than 0 when collectable_tge_percentage is not 100"
                        .to_string(),
                ));
            }
            // This can't over flow because all values are u64
            let end_timestamp: u128 =
                u128::from(start) + u128::from(cliff_duration) + u128::from(vesting_duration);
            if end_timestamp > Timestamp::MAX.into() {
                return Err(AzAirdropError::UnprocessableEntity(
                    "Combination of start, cliff_duration and vesting_duration exceeds limit"
                        .to_string(),
                ));
            }

            Ok(())
        }
    }
}
