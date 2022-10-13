use {log::*, std::collections::HashSet};

#[derive(Debug)]
pub struct AccountsSelector {
    pub accounts: HashSet<Vec<u8>>,
    pub owners: HashSet<Vec<u8>>,
    pub select_all_accounts: bool,
}

impl Default for AccountsSelector {
    fn default() -> Self {
        Self {
            accounts: HashSet::default(),
            owners: HashSet::default(),
            select_all_accounts: true,
        }
    }
}

impl AccountsSelector {
    pub fn new(accounts: &[&str], owners: &[&str]) -> anyhow::Result<Self> {
        info!(
            "Creating AccountsSelector from accounts: {:?}, owners: {:?}",
            accounts, owners
        );

        let select_all_accounts = accounts.iter().any(|key| *key == "*");
        if select_all_accounts {
            return Ok(Self::default());
        }

        Ok(AccountsSelector {
            accounts: accounts
                .iter()
                .map(|key| bs58::decode(key).into_vec())
                .collect::<Result<_, _>>()?,
            owners: owners
                .iter()
                .map(|key| bs58::decode(key).into_vec())
                .collect::<Result<_, _>>()?,
            select_all_accounts: false,
        })
    }

    pub fn from_config(config: &serde_json::Value) -> anyhow::Result<AccountsSelector> {
        Ok(if config.is_null() {
            AccountsSelector::default()
        } else {
            let accounts = &config["accounts"];
            let accounts = accounts
                .as_array()
                .map(|accounts| {
                    accounts
                        .iter()
                        .map(|account| {
                            account.as_str().ok_or_else(|| {
                                anyhow::anyhow!("Expected `account` Pubkey as String")
                            })
                        })
                        .collect::<Result<Vec<&str>, _>>()
                })
                .transpose()?
                .unwrap_or_default();

            let owners = &config["owners"];
            let owners = owners
                .as_array()
                .map(|owners| {
                    owners
                        .iter()
                        .map(|account| {
                            account
                                .as_str()
                                .ok_or_else(|| anyhow::anyhow!("Expected `owner` Pubkey as String"))
                        })
                        .collect::<Result<Vec<&str>, _>>()
                })
                .transpose()?
                .unwrap_or_default();

            AccountsSelector::new(&accounts, &owners)?
        })
    }

    pub fn is_account_selected(&self, account: &[u8], owner: &[u8]) -> bool {
        self.select_all_accounts || self.accounts.contains(account) || self.owners.contains(owner)
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    #[test]
    fn test_create_accounts_selector() {
        AccountsSelector::new(&["9xQeWvG816bUx9EPjHmaT23yvVM2ZWbrrpZb9PusVFin"], &[]).unwrap();

        AccountsSelector::new(&[], &["9xQeWvG816bUx9EPjHmaT23yvVM2ZWbrrpZb9PusVFin"]).unwrap();
    }
}
