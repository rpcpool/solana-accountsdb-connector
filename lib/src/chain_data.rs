use {
    solana_sdk::account::AccountSharedData, solana_sdk::pubkey::Pubkey, std::collections::HashMap,
};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SlotStatus {
    Rooted,
    Confirmed,
    Processed,
}

#[derive(Clone, Debug)]
pub struct SlotData {
    pub slot: u64,
    pub parent: Option<u64>,
    pub status: SlotStatus,
    pub chain: u64, // the top slot that this is in a chain with. uncles will have values < tip
}

#[derive(Clone, Debug)]
pub struct AccountData {
    pub slot: u64,
    pub write_version: u64,
    pub account: AccountSharedData,
}

/// Track slots and account writes
///
/// - use account() to retrieve the current best data for an account.
/// - update_from_snapshot() and update_from_websocket() update the state for new messages
#[derive(Debug, Default)]
pub struct ChainData {
    /// only slots >= newest_rooted_slot are retained
    slots: HashMap<u64, SlotData>,
    /// writes to accounts, only the latest rooted write an newer are retained
    accounts: HashMap<Pubkey, Vec<AccountData>>,
    newest_rooted_slot: u64,
    newest_processed_slot: u64,
}

impl ChainData {
    pub fn update_slot(&mut self, new_slot: SlotData) {
        let new_processed_head = new_slot.slot > self.newest_processed_slot;
        if new_processed_head {
            self.newest_processed_slot = new_slot.slot;
        }

        let new_rooted_head =
            new_slot.slot > self.newest_rooted_slot && new_slot.status == SlotStatus::Rooted;
        if new_rooted_head {
            self.newest_rooted_slot = new_slot.slot;
        }

        let mut parent_update = false;

        use std::collections::hash_map::Entry;
        match self.slots.entry(new_slot.slot) {
            Entry::Vacant(v) => {
                v.insert(new_slot);
            }
            Entry::Occupied(o) => {
                let v = o.into_mut();
                parent_update = v.parent != new_slot.parent && new_slot.parent.is_some();
                v.parent = v.parent.or(new_slot.parent);
                v.status = new_slot.status;
            }
        };

        if new_processed_head || parent_update {
            // update the "chain" field down to the first rooted slot
            let mut slot = self.newest_processed_slot;
            loop {
                if let Some(data) = self.slots.get_mut(&slot) {
                    data.chain = self.newest_processed_slot;
                    if data.status == SlotStatus::Rooted {
                        break;
                    }
                    if let Some(parent) = data.parent {
                        slot = parent;
                        continue;
                    }
                }
                break;
            }
        }

        if new_rooted_head {
            // for each account, preserve only writes > newest_rooted_slot, or the newest
            // rooted write
            for (_, writes) in self.accounts.iter_mut() {
                let newest_rooted_write = writes
                    .iter()
                    .rev()
                    .find(|w| {
                        w.slot <= self.newest_rooted_slot
                            && self
                                .slots
                                .get(&w.slot)
                                .map(|s| {
                                    // sometimes we seem not to get notifications about slots
                                    // getting rooted, hence assume non-uncle slots < newest_rooted_slot
                                    // are rooted too
                                    s.status == SlotStatus::Rooted
                                        || s.chain == self.newest_processed_slot
                                })
                                // preserved account writes for deleted slots <= newest_rooted_slot
                                // are expected to be rooted
                                .unwrap_or(true)
                    })
                    .map(|w| w.slot)
                    // no rooted write found: produce no effect, since writes > newest_rooted_slot are retained anyway
                    .unwrap_or(self.newest_rooted_slot + 1);
                writes
                    .retain(|w| w.slot == newest_rooted_write || w.slot > self.newest_rooted_slot);
            }

            // now it's fine to drop any slots before the new rooted head
            // as account writes for non-rooted slots before it have been dropped
            self.slots.retain(|s, _| *s >= self.newest_rooted_slot);
        }
    }

    pub fn update_account(&mut self, pubkey: Pubkey, account: AccountData) {
        use std::collections::hash_map::Entry;
        match self.accounts.entry(pubkey) {
            Entry::Vacant(v) => {
                v.insert(vec![account]);
            }
            Entry::Occupied(o) => {
                let v = o.into_mut();
                // v is ordered by slot ascending. find the right position
                // overwrite if an entry for the slot already exists, otherwise insert
                let rev_pos = v
                    .iter()
                    .rev()
                    .position(|d| d.slot <= account.slot)
                    .unwrap_or(v.len());
                let pos = v.len() - rev_pos;
                if pos < v.len() && v[pos].slot == account.slot {
                    if v[pos].write_version < account.write_version {
                        v[pos] = account;
                    }
                } else {
                    v.insert(pos, account);
                }
            }
        };
    }

    fn is_account_write_live(&self, write: &AccountData) -> bool {
        self.slots
            .get(&write.slot)
            // either the slot is rooted or in the current chain
            .map(|s| s.status == SlotStatus::Rooted || s.chain == self.newest_processed_slot)
            // if the slot can't be found but preceeds newest rooted, use it too (old rooted slots are removed)
            .unwrap_or(
                write.slot <= self.newest_rooted_slot || write.slot > self.newest_processed_slot,
            )
    }

    /// Cloned snapshot of all the most recent live writes per pubkey
    pub fn accounts_snapshot(&self) -> HashMap<Pubkey, AccountData> {
        self.accounts
            .iter()
            .filter_map(|(pubkey, writes)| {
                let latest_good_write = writes
                    .iter()
                    .rev()
                    .find(|w| self.is_account_write_live(w))?;
                Some((*pubkey, latest_good_write.clone()))
            })
            .collect()
    }

    /// Ref to the most recent live write of the pubkey
    pub fn account<'a>(&'a self, pubkey: &Pubkey) -> anyhow::Result<&'a AccountData> {
        self.accounts
            .get(pubkey)
            .ok_or_else(|| anyhow::anyhow!("account {} not found", pubkey))?
            .iter()
            .rev()
            .find(|w| self.is_account_write_live(w))
            .ok_or_else(|| anyhow::anyhow!("account {} has no live data", pubkey))
    }
}
