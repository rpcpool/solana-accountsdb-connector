use {
    crate::accounts_selector::{AccountsSelector, AccountsSelectorConfig},
    log::*,
    proto::{
        accounts_db_server::AccountsDb, update::UpdateOneof, SubscribeRequest, SubscribeResponse,
        Update, UpdateAccountsSelectorRequest, UpdateAccountsSelectorResponse,
    },
    serde::Deserialize,
    std::sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
    tokio::sync::{broadcast, mpsc},
    tokio_stream::wrappers::ReceiverStream,
    tonic::{Code, Request, Response, Result as TonicResult, Status},
};

pub mod proto {
    tonic::include_proto!("accountsdb");
}

#[derive(Debug, Clone, Deserialize)]
pub struct ServiceConfig {
    broadcast_buffer_size: usize,
    subscriber_buffer_size: usize,
}

#[derive(Debug)]
pub struct Service {
    pub config: ServiceConfig,
    pub subscribe_id: AtomicU64,
    pub highest_write_slot: Arc<AtomicU64>,
    pub updates_tx: broadcast::Sender<Update>,
    pub accounts_selector_tx: mpsc::UnboundedSender<AccountsSelector>,
}

impl Service {
    pub fn new(
        config: ServiceConfig,
        highest_write_slot: Arc<AtomicU64>,
        accounts_selector_tx: mpsc::UnboundedSender<AccountsSelector>,
    ) -> Self {
        let (updates_tx, _) = broadcast::channel(config.broadcast_buffer_size);
        Self {
            config,
            subscribe_id: AtomicU64::new(0),
            highest_write_slot,
            updates_tx,
            accounts_selector_tx,
        }
    }
}

#[tonic::async_trait]
impl AccountsDb for Service {
    type SubscribeStream = ReceiverStream<TonicResult<Update>>;

    async fn subscribe(
        &self,
        _request: Request<SubscribeRequest>,
    ) -> TonicResult<Response<Self::SubscribeStream>> {
        let id = self.subscribe_id.fetch_add(1, Ordering::SeqCst);
        info!("{}, new subscriber", id);

        let mut updates_rx = self.updates_tx.subscribe();

        let (tx, rx) = mpsc::channel(self.config.subscriber_buffer_size);
        let _ = tx.try_send(Ok(Update {
            update_oneof: Some(UpdateOneof::SubscribeResponse(SubscribeResponse {
                highest_write_slot: self.highest_write_slot.load(Ordering::Relaxed),
            })),
        }));

        tokio::spawn(async move {
            let mut exit = false;
            while !exit {
                let fwd = updates_rx.recv().await.map_err(|error| {
                    // Note: If we can't keep up pulling from the broadcast
                    // channel here, there'll be a Lagged error, and we'll
                    // close the connection because data was lost.
                    warn!(
                        "{}, error while receiving message to be broadcast: {:?}",
                        id, error
                    );
                    exit = true;
                    Status::new(Code::Internal, error.to_string())
                });
                if let Err(_err) = tx.send(fwd).await {
                    info!("{}, subscriber stream closed", id);
                    exit = true;
                }
            }
        });

        Ok(Response::new(ReceiverStream::new(rx)))
    }

    async fn update_accounts_selector(
        &self,
        request: Request<UpdateAccountsSelectorRequest>,
    ) -> TonicResult<Response<UpdateAccountsSelectorResponse>> {
        let (is_ok, error_message) =
            match serde_json::from_str::<AccountsSelectorConfig>(&request.get_ref().config)
                .map_err(|error| error.to_string())
                .and_then(|config| {
                    AccountsSelector::from_config(&config).map_err(|error| error.to_string())
                }) {
                Ok(accounts_selector) => match self.accounts_selector_tx.send(accounts_selector) {
                    Ok(()) => (true, String::new()),
                    Err(error) => (false, error.to_string()),
                },
                Err(error) => (false, error),
            };

        Ok(Response::new(UpdateAccountsSelectorResponse {
            is_ok,
            error_message,
        }))
    }
}
