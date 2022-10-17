use {log::*, std::collections::HashSet};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccountsFilter {
    select_all_accounts: bool,
    accounts: HashSet<Vec<u8>>,
    owners: HashSet<Vec<u8>>,
}

impl Default for AccountsFilter {
    fn default() -> Self {
        Self {
            accounts: HashSet::default(),
            owners: HashSet::default(),
            select_all_accounts: true,
        }
    }
}

impl AccountsFilter {
    pub fn new<T1, T2>(accounts: &[T1], owners: &[T2]) -> anyhow::Result<Self>
    where
        for<'a> T1: AsRef<[u8]> + std::cmp::PartialEq<&'a str> + std::fmt::Debug,
        T2: AsRef<[u8]> + std::fmt::Debug,
    {
        info!(
            "Creating AccountsFilter from accounts: {:?}, owners: {:?}",
            accounts, owners
        );

        let select_all_accounts = accounts.iter().any(|key| *key == "*");
        if select_all_accounts {
            return Ok(Self::default());
        }

        Ok(AccountsFilter {
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

    pub fn is_account_selected(&self, account: &[u8], owner: &[u8]) -> bool {
        self.select_all_accounts || self.accounts.contains(account) || self.owners.contains(owner)
    }
}
