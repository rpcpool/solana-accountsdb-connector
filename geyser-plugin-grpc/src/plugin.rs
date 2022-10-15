use {
    crate::{
        accounts_selector::{AccountsSelector, AccountsSelectorConfig},
        prom::{
            PrometheusConfig, PrometheusService, BROADCAST_ACCOUNTS_TOTAL, BROADCAST_SLOTS_TOTAL,
            SLOTS_LAST_PROCESSED,
        },
        service::{
            proto::{
                accounts_db_server::AccountsDbServer, slot_update::Status as SlotUpdateStatus,
                update::UpdateOneof, AccountWrite, Ping, SlotUpdate, Update,
            },
            Service, ServiceConfig,
        },
    },
    log::*,
    serde::Deserialize,
    solana_geyser_plugin_interface::geyser_plugin_interface::{
        GeyserPlugin, GeyserPluginError, ReplicaAccountInfoVersions, Result as PluginResult,
        SlotStatus,
    },
    std::{
        collections::HashSet,
        convert::TryInto,
        fs::read_to_string,
        net::SocketAddr,
        sync::{
            atomic::{AtomicU64, Ordering},
            Arc,
        },
    },
    tokio::{
        runtime::Runtime,
        sync::{broadcast, mpsc},
        time::{sleep, Duration},
    },
    tonic::{codec::CompressionEncoding, transport::Server},
};

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PluginConfig {
    pub bind_address: SocketAddr,
    pub accounts_selector: AccountsSelectorConfig,
    pub service_config: ServiceConfig,
    pub zstd_compression: bool,
    #[serde(default)]
    pub prometheus: Option<PrometheusConfig>,
}

#[derive(Debug)]
pub struct PluginInner {
    runtime: tokio::runtime::Runtime,
    zstd_compression: bool,
    accounts_selector: AccountsSelector,
    accounts_selector_rx: mpsc::UnboundedReceiver<AccountsSelector>,
    /// Largest slot that an account write was processed for
    highest_write_slot: Arc<AtomicU64>,
    /// Accounts that saw account writes
    /// Needed to catch writes that signal account closure, where
    /// lamports=0 and owner=system-program.
    active_accounts: HashSet<[u8; 32]>,
    server_broadcast_tx: broadcast::Sender<Update>,
    server_exit_sender: broadcast::Sender<()>,
    prometheus: PrometheusService,
}

impl PluginInner {
    fn broadcast(&self, update: UpdateOneof) {
        // Don't care about the error that happens when there are no receivers.
        let _ = self.server_broadcast_tx.send(Update {
            update_oneof: Some(update),
        });
    }
}

#[derive(Debug, Default)]
pub struct Plugin {
    inner: Option<PluginInner>,
}

impl GeyserPlugin for Plugin {
    fn name(&self) -> &'static str {
        "GeyserPluginGrpc"
    }

    fn on_load(&mut self, config_file: &str) -> PluginResult<()> {
        solana_logger::setup_with_default("info");
        info!(
            "Loading plugin {:?} from config_file {:?}",
            self.name(),
            config_file
        );

        let runtime = Runtime::new().unwrap();

        let config = read_to_string(config_file).map_err(GeyserPluginError::ConfigFileOpenError)?;
        let config: PluginConfig = serde_json::from_str(&config).map_err(|error| {
            GeyserPluginError::ConfigFileReadError {
                msg: format!("Failed to read config from the file: {:?}", error),
            }
        })?;

        let (accounts_selector_tx, accounts_selector_rx) = mpsc::unbounded_channel();
        let accounts_selector =
            AccountsSelector::from_config(&config.accounts_selector).map_err(|error| {
                GeyserPluginError::ConfigFileReadError {
                    msg: format!("Failed to create accounts filter: {:?}", error),
                }
            })?;

        let highest_write_slot = Arc::new(AtomicU64::new(0));

        let service = Service::new(
            config.service_config,
            Arc::clone(&highest_write_slot),
            accounts_selector_tx,
        );
        let server_broadcast_tx = service.updates_tx.clone();
        let (server_exit_sender, _) = broadcast::channel::<()>(1);

        let mut server_exit_receiver = server_exit_sender.subscribe();
        runtime.spawn(
            Server::builder()
                .add_service(
                    AccountsDbServer::new(service)
                        .accept_compressed(CompressionEncoding::Gzip)
                        .send_compressed(CompressionEncoding::Gzip),
                )
                .serve_with_shutdown(config.bind_address, async move {
                    let _ = server_exit_receiver.recv().await;
                }),
        );

        let server_broadcast_tx_ping = server_broadcast_tx.clone();
        let mut server_exit_receiver = server_exit_sender.subscribe();
        runtime.spawn(async move {
            loop {
                // Don't care about the error if there are no receivers.
                let _ = server_broadcast_tx_ping.send(Update {
                    update_oneof: Some(UpdateOneof::Ping(Ping {})),
                });

                tokio::select! {
                    _ = server_exit_receiver.recv() => { break; },
                    _ = sleep(Duration::from_secs(5)) => {},
                }
            }
        });

        let prometheus = PrometheusService::new(&runtime, config.prometheus);

        self.inner = Some(PluginInner {
            runtime,
            zstd_compression: config.zstd_compression,
            accounts_selector,
            accounts_selector_rx,
            highest_write_slot,
            active_accounts: HashSet::new(),
            server_broadcast_tx,
            server_exit_sender,
            prometheus,
        });

        Ok(())
    }

    fn on_unload(&mut self) {
        info!("Unloading plugin: {:?}", self.name());

        let data = self.inner.take().expect("plugin must be initialized");

        data.prometheus.shutdown();
        let _ = data.server_exit_sender.send(());
        data.runtime.shutdown_background();
    }

    fn update_account(
        &mut self,
        account: ReplicaAccountInfoVersions,
        slot: u64,
        is_startup: bool,
    ) -> PluginResult<()> {
        let inner = self.inner.as_mut().expect("plugin must be initialized");
        match account {
            ReplicaAccountInfoVersions::V0_0_1(account) => {
                // Select only accounts configured to look at, plus writes to accounts
                // that were previously selected (to catch closures and account reuse)
                let is_selected = inner
                    .accounts_selector
                    .is_account_selected(account.pubkey, account.owner);
                let previously_selected = inner.active_accounts.contains(&account.pubkey[0..32]);
                if !is_selected && !previously_selected {
                    return Ok(());
                }
                if !previously_selected {
                    inner
                        .active_accounts
                        .insert(account.pubkey.try_into().unwrap());
                }

                inner.highest_write_slot.fetch_max(slot, Ordering::Relaxed);

                // zstd compress if enabled
                let data = if inner.zstd_compression {
                    match zstd::encode_all(account.data, 0) {
                        Ok(vec) => vec,
                        Err(error) => {
                            error!("zstd_decompress compression failed: {:?}", error);
                            account.data.to_vec()
                        }
                    }
                } else {
                    account.data.to_vec()
                };

                inner.broadcast(UpdateOneof::AccountWrite(AccountWrite {
                    slot,
                    pubkey: account.pubkey.to_vec(),
                    lamports: account.lamports,
                    owner: account.owner.to_vec(),
                    executable: account.executable,
                    rent_epoch: account.rent_epoch,
                    data,
                    write_version: account.write_version,
                    is_startup,
                    is_selected,
                }));

                BROADCAST_ACCOUNTS_TOTAL.inc();
            }
        }

        Ok(())
    }

    fn update_slot_status(
        &mut self,
        slot: u64,
        parent: Option<u64>,
        status: SlotStatus,
    ) -> PluginResult<()> {
        let inner = self.inner.as_mut().expect("plugin must be initialized");

        if status == SlotStatus::Confirmed {
            while let Ok(accounts_selector) = inner.accounts_selector_rx.try_recv() {
                inner.accounts_selector = accounts_selector;
            }
        }

        let (status, label) = match status {
            SlotStatus::Processed => (SlotUpdateStatus::Processed, "processed"),
            SlotStatus::Confirmed => (SlotUpdateStatus::Confirmed, "confirmed"),
            SlotStatus::Rooted => (SlotUpdateStatus::Rooted, "rooted"),
        };

        inner.broadcast(UpdateOneof::SlotUpdate(SlotUpdate {
            slot,
            parent,
            status: status as i32,
        }));

        SLOTS_LAST_PROCESSED
            .with_label_values(&[label])
            .set(slot as i64);
        BROADCAST_SLOTS_TOTAL.with_label_values(&[label]).inc();

        Ok(())
    }
}

#[no_mangle]
#[allow(improper_ctypes_definitions)]
/// # Safety
///
/// This function returns the Plugin pointer as trait GeyserPlugin.
pub unsafe extern "C" fn _create_plugin() -> *mut dyn GeyserPlugin {
    let plugin = Plugin::default();
    let plugin: Box<dyn GeyserPlugin> = Box::new(plugin);
    Box::into_raw(plugin)
}
