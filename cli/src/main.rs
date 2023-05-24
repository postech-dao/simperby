use clap::Parser;
use eyre::eyre;
use simperby::{types::*, Client};
use simperby_cli::cli::{self, Commands, CreateCommands, SignCommands};
use simperby_core::{utils::get_timestamp, *};
use simperby_repository::{
    raw::RawRepository,
    server::{build_simple_git_server, PushVerifier},
};

async fn run(
    args: cli::Cli,
    path: String,
    config: Config,
    auth: Auth,
    server_config: Option<ServerConfig>,
) -> eyre::Result<()> {
    match args.command {
        Commands::Genesis => Client::genesis(&path).await,
        Commands::Init => Client::init(&path).await,
        Commands::Clone { url } => {
            RawRepository::clone(&path, &url).await?;
            std::env::set_current_dir(path.clone())?;
            Client::init(&path).await
        }
        Commands::Network => todo!("network is not implemented yet"),
        Commands::Chat { .. } => todo!("chat is not implemented yet"),
        Commands::Sign(SignCommands::TxDelegate {
            delegator,
            delegatee,
            governance,
            target_height,
            chain_name,
        }) => {
            let delegation_transaction_data = DelegationTransactionData {
                delegator,
                delegatee,
                governance,
                block_height: target_height,
                timestamp: get_timestamp(),
                chain_name,
            };
            TypedSignature::<DelegationTransactionData>::sign(
                &delegation_transaction_data,
                &auth.private_key,
            )
            .map_err(|_| eyre!("failed to sign"))
            .map(|signature| {
                println!("{:?}", serde_spb::to_string(&signature));
            })
        }
        Commands::Sign(SignCommands::TxUndelegate {
            delegator,
            target_height,
            chain_name,
        }) => {
            let undelegation_transaction_data = UndelegationTransactionData {
                delegator,
                block_height: target_height,
                timestamp: get_timestamp(),
                chain_name,
            };
            TypedSignature::<UndelegationTransactionData>::sign(
                &undelegation_transaction_data,
                &auth.private_key,
            )
            .map_err(|_| eyre!("failed to sign"))
            .map(|signature| {
                println!("{:?}", serde_spb::to_string(&signature));
            })
        }
        Commands::Sign(SignCommands::Custom { hash }) => {
            let hash = Hash256::from_array(
                hex::decode(hash)?
                    .as_slice()
                    .try_into()
                    .map_err(|_| eyre!("a hash must be in 32 bytes"))?,
            );
            Signature::sign(hash, &auth.private_key)
                .map_err(|_| eyre!("failed to sign"))
                .map(|signature| {
                    println!("{:?}", serde_spb::to_string(&signature));
                })
        }
        Commands::CheckPush { .. } => todo!("check push is not implemented yet"),
        Commands::NotifyPush { .. } => todo!("notify push is not implemented yet"),
        // Commands that require `open` to be called.
        _ => {
            let mut client = Client::open(&path, config, auth.clone()).await?;
            match args.command {
                Commands::Clean { hard } => client.clean(hard).await,
                Commands::Create(CreateCommands::TxDelegate {
                    delegator,
                    delegatee,
                    governance,
                    block_height,
                    proof,
                    chain_name,
                }) => client
                    .repository_mut()
                    .create_extra_agenda_transaction(&ExtraAgendaTransaction::Delegate(
                        TxDelegate {
                            data: DelegationTransactionData {
                                delegator: serde_spb::from_str(&delegator).map_err(|_| {
                                    eyre!("invalid delegator for a delegation transaction")
                                })?,
                                delegatee: serde_spb::from_str(&delegatee).map_err(|_| {
                                    eyre!("invalid delegatee for a delegation transaction")
                                })?,
                                governance,
                                block_height,
                                timestamp: get_timestamp(),
                                chain_name,
                            },
                            proof: serde_spb::from_str(&proof)
                                .map_err(|_| eyre!("invalid proof for a delegation transaction"))?,
                        },
                    ))
                    .await
                    .map_or_else(
                        |err| Err(eyre!("failed to create a delegation transaction: {}", err)),
                        |_| Ok(()),
                    ),
                Commands::Create(CreateCommands::TxUndelegate {
                    delegator,
                    block_height,
                    proof,
                    chain_name,
                }) => client
                    .repository_mut()
                    .create_extra_agenda_transaction(&ExtraAgendaTransaction::Undelegate(
                        TxUndelegate {
                            data: UndelegationTransactionData {
                                delegator: serde_spb::from_str(&delegator).map_err(|_| {
                                    eyre!("invalid delegator for an undelegation transaction")
                                })?,
                                block_height,
                                timestamp: get_timestamp(),
                                chain_name,
                            },
                            proof: serde_spb::from_str(&proof).map_err(|_| {
                                eyre!("invalid proof for an undelegation transaction")
                            })?,
                        },
                    ))
                    .await
                    .map_or_else(
                        |err| {
                            Err(eyre!(
                                "failed to create an undelegation transaction: {}",
                                err
                            ))
                        },
                        |_| Ok(()),
                    ),
                Commands::Create(CreateCommands::TxReport) => {
                    todo!("TxReport is not implemented yet")
                }
                Commands::Create(CreateCommands::Block) => client
                    .repository_mut()
                    .create_block(auth.private_key.public_key())
                    .await
                    .map_or_else(
                        |err| Err(eyre!("failed to create a block: {}", err)),
                        |_| Ok(()),
                    ),
                Commands::Create(CreateCommands::Agenda) => {
                    let reserved_state = client
                        .repository()
                        .get_raw()
                        .blocking_read()
                        .read_reserved_state()
                        .await?;
                    let name = reserved_state
                        .query_name(&auth.private_key.public_key())
                        .ok_or(eyre!("member does not exist with the public key"))?;
                    client
                        .repository_mut()
                        .create_agenda(name)
                        .await
                        .map_or_else(
                            |err| Err(eyre!("failed to create an agenda: {}", err)),
                            |_| Ok(()),
                        )
                }
                Commands::Vote { revision } => {
                    let commit_hash = client
                        .repository()
                        .get_raw()
                        .blocking_read()
                        .retrieve_commit_hash(revision)
                        .await?;
                    client
                        .vote(commit_hash)
                        .await
                        .map_or_else(|err| Err(eyre!("failed to vote: {}", err)), |_| Ok(()))
                }
                Commands::Veto { revision } => {
                    if revision.is_none() {
                        client.veto_round().await
                    } else {
                        let commit_hash = client
                            .repository()
                            .get_raw()
                            .blocking_read()
                            .retrieve_commit_hash(revision.expect("revision is not none"))
                            .await?;
                        client.veto_block(commit_hash).await
                    }
                }
                Commands::Consensus { show } => {
                    if show {
                        todo!("showing the status of the consensus is not implemented yet")
                    } else {
                        let result = client.progress_for_consensus().await;
                        match result {
                            Ok(_) => Ok(()),
                            Err(err) => {
                                Err(eyre!("failed to make a progress for consensus: {}", err))
                            }
                        }
                    }
                }
                Commands::Show { revision } => {
                    let commit_hash = client
                        .repository()
                        .get_raw()
                        .blocking_read()
                        .retrieve_commit_hash(revision)
                        .await?;
                    println!("{:?}", client.show(commit_hash).await?);
                    Ok(())
                }
                Commands::Serve => {
                    if let Some(server_config) = server_config {
                        client
                            .serve(
                                server_config,
                                PushVerifier::VerifierExecutable(build_simple_git_server()),
                            )
                            .await?
                            .await?
                    } else {
                        Err(eyre!("server config is not provided"))
                    }
                }
                Commands::Update { _no_network } => client.update().await,
                Commands::Broadcast => client.broadcast().await,
                _ => unimplemented!("not implemented yet"),
            }
        }
    }
}

#[tokio::main(flavor = "multi_thread")]
#[allow(unreachable_code)]
async fn main() -> eyre::Result<()> {
    color_eyre::install().unwrap();
    env_logger::init();

    let args = cli::Cli::parse();
    let path = args.path.display().to_string();
    let config: Config =
        serde_spb::from_str(&tokio::fs::read_to_string(&format!("{path}/config.json")).await?)?;
    let auth: Auth = serde_spb::from_str(
        &tokio::fs::read_to_string(&format!("{path}/.simperby/auth.json")).await?,
    )?;
    let server_config: Option<ServerConfig> = serde_spb::from_str(
        &tokio::fs::read_to_string(&format!("{path}/.simperby/server_config.json")).await?,
    )?;

    if let Err(e) = run(args, path, config, auth, server_config).await {
        if let Ok(_err) = e.downcast::<simperby::simperby_repository::IntegrityError>() {
            // TODO: perform some special handling?
        }
    }

    Ok(())
}
