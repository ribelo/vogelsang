use serde::Deserialize;

#[allow(dead_code)]
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Account {
    pub address: Address,
    pub bank_account: BankAccount,
    pub can_upgrade: bool,
    pub cellphone_number: String,
    pub client_role: String,
    pub contract_type: String,
    pub culture: String,
    pub display_language: String,
    pub display_name: String,
    pub effective_client_role: String,
    pub email: String,
    pub first_contact: FirstContact,
    pub flatex_bank_account: FlatexBankAccount,
    pub id: i32,
    pub int_account: i32,
    pub is_allocation_available: bool,
    pub is_am_client_active: bool,
    pub is_collective_portfolio: bool,
    pub is_isk_client: bool,
    pub is_withdrawal_available: bool,
    pub language: String,
    pub locale: String,
    pub logged_in_person_id: i32,
    pub member_code: String,
    pub username: String,
    pub info: Option<AccountInfo>,
}

#[allow(dead_code)]
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountInfo {
    pub base_currency: String,
    pub margin_type: String,
}


#[allow(dead_code)]
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Address {
    pub city: String,
    pub country: String,
    pub street_address: String,
    pub street_address_ext: String,
    pub street_address_number: String,
    pub zip: String,
}

#[allow(dead_code)]
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BankAccount {
    pub bank_account_id: i32,
    pub bic: String,
    pub iban: String,
    pub status: String,
}

#[allow(dead_code)]
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FirstContact {
    pub country_of_birth: String,
    pub date_of_birth: String,
    pub display_name: String,
    pub first_name: String,
    pub gender: String,
    pub last_name: String,
    pub nationality: String,
    pub place_of_birth: String,
}

#[allow(dead_code)]
#[derive(Clone, Debug, Default, Deserialize)]
pub struct FlatexBankAccount {
    pub bic: String,
    pub iban: String,
}
