use ink::{
    env::Error as InkEnvError,
    prelude::{format, string::String},
    LangError,
};

#[derive(Debug, PartialEq, Eq, scale::Encode, scale::Decode)]
#[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
pub enum AzTokenSaleToAirdropError {
    ContractCall(LangError),
    InkEnvError(String),
    NotFound(String),
    Unauthorised,
    UnprocessableEntity(String),
}
impl From<InkEnvError> for AzTokenSaleToAirdropError {
    fn from(e: InkEnvError) -> Self {
        AzTokenSaleToAirdropError::InkEnvError(format!("{e:?}"))
    }
}
impl From<LangError> for AzTokenSaleToAirdropError {
    fn from(e: LangError) -> Self {
        AzTokenSaleToAirdropError::ContractCall(e)
    }
}
