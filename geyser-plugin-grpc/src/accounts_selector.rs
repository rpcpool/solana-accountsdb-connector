use {log::*, serde::Deserialize, std::collections::HashSet};

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct AccountsSelectorConfig {
    accounts: Vec<String>,
    owners: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
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
    pub fn new<T1, T2>(accounts: &[T1], owners: &[T2]) -> anyhow::Result<Self>
    where
        for<'a> T1: AsRef<[u8]> + std::cmp::PartialEq<&'a str> + std::fmt::Debug,
        T2: AsRef<[u8]> + std::fmt::Debug,
    {
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

    pub fn from_config(config: &AccountsSelectorConfig) -> anyhow::Result<AccountsSelector> {
        Self::new(&config.accounts, &config.owners)
    }

    pub fn is_account_selected(&self, account: &[u8], owner: &[u8]) -> bool {
        self.select_all_accounts || self.accounts.contains(account) || self.owners.contains(owner)
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::{AccountsSelector, AccountsSelectorConfig};

    #[test]
    fn test_create_accounts_selector() {
        assert!(AccountsSelector::from_config(&AccountsSelectorConfig {
            accounts: vec!["9xQeWvG816bUx9EPjHmaT23yvVM2ZWbrrpZb9PusVFin".to_owned()],
            owners: vec![],
        })
        .is_ok());

        assert!(AccountsSelector::from_config(&AccountsSelectorConfig {
            accounts: vec![],
            owners: vec!["9xQeWvG816bUx9EPjHmaT23yvVM2ZWbrrpZb9PusVFin".to_owned()],
        })
        .is_ok());
    }
}
