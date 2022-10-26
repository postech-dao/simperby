use clap::{Parser, Subcommand};

/**
Welcome to the Simperby CLI!
*/
#[derive(Debug, Parser)]
#[clap(name = "git")]
#[clap(about = "A Simperby client CLI", long_about = None)]
pub struct Cli {
    #[clap(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
pub enum CreateCommands {
    /// An extra-agenda transaction that delegates the consensus voting power.
    TxDelegate {
        delegator: String,
        delegatee: String,
        /// Whether to delegate the governance voting power too.
        governance: bool,
        proof: String,
    },
    /// An extra-agenda transaction that undelegates the consensus voting power and
    /// the governance voting power (if delegated).
    TxUndelegate { delegator: String, proof: String },
    /// An extra-agenda transaction that reports a misbehaving validator.
    TxReport, // TODO
    /// A block waiting for finalization.
    Block,
    /// An agenda waiting for governance approval.
    Agenda,
}

#[derive(Debug, Subcommand)]
pub enum SignCommands {
    TxDelegate {
        delegatee: String,
        /// Whether to delegate the governance voting power too.
        governance: bool,
        target_height: u64,
    },
    TxUndelegate {
        target_height: u64,
    },
    Custom {
        message: String,
    },
}

#[derive(Debug, Subcommand)]
pub enum Commands {
    /// Initialize a new Simperby node in the current directory.
    Init,
    /// Sync the `finalized` branch to the given commit.
    ///
    /// This will verify every commit along the way.
    /// If the given commit is not a descendant of the
    /// current `finalized` (i.e., cannot be fast-forwarded), it fails.
    ///
    /// Note that if you sync to a block `H`, then the `finalized` branch will move to `H-1`.
    /// To sync the last block `H`, you have to run `update`.
    /// (This is because the finalization proof for a block appears in the next block.)
    Sync { commit: String },
    /// Print the information about the block data Git server that this node is hosting.
    Git,
    /// Clean the repository, removing all the outdated (incompatible with `finalized`) commits.
    Clean {
        /// If enabled, it will remove all the commits except those included in `finalized`.
        #[clap(long, action)]
        hard: bool,
    },
    /// Create a new commit on top of the `work` branch.
    #[command(subcommand)]
    Create(CreateCommands),
    /// Vote on the agenda, broadcasting to the network.
    /// It will also leave a `vote` tag on the commit (with some postfix).
    Vote { commit: String },
    /// Veto the round.
    ///
    /// It will be broadcasted to the network as a nil-vote
    /// in the next `consensus`, if the conditions are met.
    /// You can check whether the round is vetoed by running `consensus --show`.
    Veto {
        /// If specified, it vetoes a specific block proposal,
        /// leaving a `veto` tag on the commit (with some postfix).
        /// It fails if the given commit is already set to `proposal`.
        /// If the given commit is already set to `veto`, it will be removed.
        commit: Option<String>,
    },
    /// Show the governance status of the given agenda.
    Show { commit: String },
    /// Run the Simperby node indefinitely. This is same as running `relay` while
    /// invoking `consensus` and `fetch` repeatedly.
    Run,
    /// Make a progress on the consensus.
    ///
    /// The node may broadcast the proposal or consensus messages depending on the
    /// current consensus round state.
    Consensus {
        /// If enabled, it shows the status of the consensus.
        ///
        /// Unlike the governance which is performed on each agenda,
        /// the consensus is 'global' so this option is not associated with any commit.
        #[clap(long, action)]
        show: bool,
    },
    /// Show the current status of the p2p network.
    Network,
    /// Serve the gossip protocol indefinitely, relaying the incoming packets to other peers.
    Relay,
    /// Fetch the data broadcasted over the network and update it to the repository,
    /// the governance, and the consensus.
    Fetch,
    /// Chat on the P2P network.
    Chat {
        /// The message to say. If not specified, it prints the chat log.
        message: Option<String>,
        /// Off-the-Record. If enabled, it will not be recorded on the blockchain.
        #[clap(short, long, action)]
        otr: bool,
        /// Start an interactive chat session.
        #[clap(short, long, action)]
        interactive: bool,
    },
    /// Sign a message with the configured private key.
    #[command(subcommand)]
    Sign(SignCommands),
    /// Notify that there was a Git push on the repository.
    ///
    /// This is invoked from the Git hook.
    /// A user should not invoke this command directly.
    GitPush,
}
