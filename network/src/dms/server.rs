use super::*;

/// Runs a DMS server. This function will block the current thread.
pub async fn serve<S: Storage, M: DmsMessage>(
    dms: Arc<RwLock<DistributedMessageSet<S, M>>>,
    network_config: ServerNetworkConfig,
) -> Result<(), Error> {
    let port_key = format!("dms-{}", dms.read().await.config.dms_key);
    let port = network_config
        .ports
        .get(&port_key)
        .ok_or_else(|| eyre!(format!("`ports` has no field of {port_key}")))?;

    let rpc_task = async move {
        let wrapped_dms = Arc::new(parking_lot::RwLock::new(Some(dms)));
        let wrapped_dms_ = Arc::clone(&wrapped_dms);
        struct DropHelper<T> {
            wrapped_dms: Arc<parking_lot::RwLock<Option<Arc<RwLock<T>>>>>,
        }
        impl<T> Drop for DropHelper<T> {
            fn drop(&mut self) {
                self.wrapped_dms.write().take().unwrap();
            }
        }
        let _drop_helper = DropHelper { wrapped_dms };
        run_server(
            *port,
            [(
                "dms".to_owned(),
                create_http_object(Arc::new(DmsWrapper { dms: wrapped_dms_ })
                    as Arc<dyn DistributedMessageSetRpcInterface>),
            )]
            .iter()
            .cloned()
            .collect(),
        )
        .await;
    };
    rpc_task.await;
    Ok(())
}

/// Runs a DMS client with auto-sync. This function will block the current thread.
pub async fn sync<S: Storage, M: DmsMessage>(
    dms: Arc<RwLock<DistributedMessageSet<S, M>>>,
    fetch_interval: Option<Duration>,
    broadcast_interval: Option<Duration>,
    network_config: ClientNetworkConfig,
) -> Result<(), Error> {
    let dms_ = Arc::clone(&dms);
    let network_config_ = network_config.clone();
    let fetch_task = async move {
        if let Some(interval) = fetch_interval {
            loop {
                if let Err(e) =
                    DistributedMessageSet::<S, M>::fetch(Arc::clone(&dms_), &network_config_).await
                {
                    log::warn!("failed to parse message from the RPC-fetch: {}", e);
                }
                tokio::time::sleep(interval).await;
            }
        } else {
            futures::future::pending::<()>().await;
        }
    };
    let dms_ = Arc::clone(&dms);
    let broadcast_task = async move {
        if let Some(interval) = broadcast_interval {
            loop {
                if let Err(e) =
                    DistributedMessageSet::<S, M>::broadcast(Arc::clone(&dms_), &network_config)
                        .await
                {
                    log::warn!("failed to parse message from the RPC-broadcast: {}", e);
                }
                tokio::time::sleep(interval).await;
            }
        } else {
            futures::future::pending::<()>().await;
        }
    };
    join(fetch_task, broadcast_task).await;
    Ok(())
}
