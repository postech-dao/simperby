mod cli;
mod genesis;

use clap::Parser;
use cli::*;
use eyre::{eyre, Result};
use simperby_common::utils::get_timestamp;
use simperby_node::{
    clone, genesis, initialize, serve, simperby_common::*, simperby_repository::CommitHash,
    CommitInfo, Config,
};

fn to_commit_hash(s: &str) -> Result<CommitHash> {
    let hash = hex::decode(s).map_err(|_| eyre!("invalid hash"))?;
    let hash = hash
        .as_slice()
        .try_into()
        .map_err(|_| eyre!("a hash must be in 20 bytes"))?;
    Ok(CommitHash { hash })
}

async fn run(args: cli::Cli, path: String, config: Config) -> eyre::Result<()> {
    match args.command {
        Commands::Genesis => {
            genesis(config, &path).await?;
        }
        Commands::Init => todo!(),
        Commands::Clone { url } => {
            clone(config, &path, &url).await?;
        }
        Commands::Git => todo!(),
        Commands::Show { commit } => show(config, &path, commit).await?,
        Commands::Network => todo!(),
        Commands::Serve => {
            serve(config, &path).await?;
        }
        Commands::Chat { .. } => todo!("chat is not implemented yet"),
        Commands::Sign(SignCommands::TxDelegate {
            delegatee,
            governance,
            target_height,
        }) => {
            let delegation_transaction_data = DelegationTransactionData {
                delegator: config.public_key,
                delegatee: serde_spb::from_str(&delegatee)
                    .map_err(|_| eyre!("invalid delegatee for a delegation transaction"))?,
                governance,
                block_height: target_height,
                timestamp: get_timestamp(),
            };
            println!(
                "{:?}",
                serde_spb::to_string(
                    &TypedSignature::<DelegationTransactionData>::sign(
                        &delegation_transaction_data,
                        &config.private_key,
                    )
                    .map_err(|_| eyre!("failed to sign"))?
                )
            );
        }
        Commands::Sign(SignCommands::TxUndelegate { target_height }) => {
            let undelegation_transaction_data = UndelegationTransactionData {
                delegator: config.public_key,
                block_height: target_height,
                timestamp: get_timestamp(),
            };
            println!(
                "{:?}",
                serde_spb::to_string(
                    &TypedSignature::<UndelegationTransactionData>::sign(
                        &undelegation_transaction_data,
                        &config.private_key,
                    )
                    .map_err(|_| eyre!("failed to sign"))?
                )
            );
        }
        Commands::Sign(SignCommands::Custom { hash }) => {
            let hash = Hash256::from_array(
                hex::decode(hash)?
                    .as_slice()
                    .try_into()
                    .map_err(|_| eyre!("a hash must be in 32 bytes"))?,
            );
            println!(
                "{}",
                hex::encode(
                    Signature::sign(hash, &config.private_key)
                        .map_err(|_| eyre!("failed to sign"))?
                )
            );
        }
        Commands::CheckPush { .. } => todo!("check push is not implemented yet"),
        Commands::NotifyPush { .. } => todo!("notify push is not implemented yet"),
        // Commands that require `initialize` to be called.
        _ => {
            let mut simperby_node = initialize(config, &path).await?;
            match args.command {
                Commands::Sync {
                    last_finalization_proof,
                } => {
                    simperby_node
                        .sync(
                            serde_spb::from_str(&last_finalization_proof)
                                .map_err(|_| eyre!("invalid last finalization proof for sync"))?,
                        )
                        .await?;
                }
                Commands::Clean { hard } => {
                    simperby_node.clean(hard).await?;
                }
                Commands::Create(CreateCommands::TxDelegate {
                    delegator,
                    delegatee,
                    governance,
                    block_height,
                    proof,
                }) => {
                    simperby_node
                        .create_extra_agenda_transaction(ExtraAgendaTransaction::Delegate(
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
                                },
                                proof: serde_spb::from_str(&proof).map_err(|_| {
                                    eyre!("invalid proof for a delegation transaction")
                                })?,
                            },
                        ))
                        .await?;
                }
                Commands::Create(CreateCommands::TxUndelegate {
                    delegator,
                    block_height,
                    proof,
                }) => {
                    simperby_node
                        .create_extra_agenda_transaction(ExtraAgendaTransaction::Undelegate(
                            TxUndelegate {
                                data: UndelegationTransactionData {
                                    delegator: serde_spb::from_str(&delegator).map_err(|_| {
                                        eyre!("invalid delegator for an undelegation transaction")
                                    })?,
                                    block_height,
                                    timestamp: get_timestamp(),
                                },
                                proof: serde_spb::from_str(&proof).map_err(|_| {
                                    eyre!("invalid proof for an undelegation transaction")
                                })?,
                            },
                        ))
                        .await?;
                }
                Commands::Create(CreateCommands::TxReport) => {
                    todo!("TxReport is not implemented yet")
                }
                Commands::Create(CreateCommands::Block) => {
                    simperby_node.create_block().await?;
                }
                Commands::Create(CreateCommands::Agenda) => {
                    simperby_node.create_agenda().await?;
                }
                Commands::Vote { commit } => {
                    simperby_node
                        .vote(
                            serde_spb::from_str(&commit)
                                .map_err(|_| eyre!("invalid agenda commit hash to vote on"))?,
                        )
                        .await?;
                }
                Commands::Veto { commit } => {
                    if commit.is_none() {
                        simperby_node.veto_round().await?;
                    } else {
                        simperby_node
                            .veto_block(
                                serde_spb::from_str(&commit.expect("commit is not none"))
                                    .map_err(|_| eyre!("invalid block commit hash to veto on"))?,
                            )
                            .await?;
                    }
                }
                Commands::Consensus { show } => {
                    if show {
                        // TODO: show the status of the consensus instead of making a progress.
                    } else {
                        simperby_node.progress_for_consensus().await?;
                    }
                }
                Commands::Update => {
                    simperby_node.fetch().await?;
                }
                Commands::Broadcast => {
                    simperby_node.broadcast().await?;
                }
                _ => unreachable!("has been covered by the outer match"),
            }
        }
    }
    Ok(())
}

#[tokio::main(flavor = "multi_thread")]
#[allow(unreachable_code)]
async fn main() -> eyre::Result<()> {
    color_eyre::install().unwrap();
    env_logger::init();

    let private_key = std::env::args().nth(1).unwrap();
    let server_or_client = std::env::args().nth(2).unwrap();
    if server_or_client == "s" {
        genesis::run_genesis_proposer(&private_key).await;
    } else {
        genesis::run_genesis_non_proposer(&private_key).await;
    }

    return Ok(());

    let args = cli::Cli::parse();
    let path = args.path.display().to_string();
    let config: Config =
        serde_spb::from_str(&tokio::fs::read_to_string(&format!("{path}/config.json")).await?)?;

    if let Err(e) = run(args, path, config).await {
        if let Ok(_err) = e.downcast::<simperby_node::simperby_repository::IntegrityError>() {
            // TODO: perform some special handling?
        }
    }

    Ok(())
}

/// For every type of commit,
/// 1. Show the content.
/// 2. Show the hash of it.
///
/// For an agenda, show the governance status.
/// For a block, show the consensus status projected on this block.
/// For an extra-agenda transaction and a chat log, TODO.
async fn show(config: Config, path: &str, commit_hash: String) -> Result<()> {
    let node = simperby_node::initialize(config, path).await?;
    let result = node.show(to_commit_hash(&commit_hash)?).await?;
    match result {
        CommitInfo::Block { block_header, .. } => {
            println!("hash: {}", block_header.to_hash256());
            // TODO
        }
        _ => todo!(),
    }
    Ok(())
}
