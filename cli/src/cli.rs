use clap::{Parser, Subcommand};

/**
Welcome to the Simperby CLI!
*/
#[derive(Debug, Parser)]
#[clap(name = "git")]
#[clap(about = "A Simperby client CLI", long_about = None)]
pub struct Cli {
    pub path: std::path::PathBuf,
    #[clap(subcommand)]
    pub command: Commands,
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
        hash: String,
    },
}

#[derive(Debug, Subcommand)]
pub enum Commands {
    /// Clone a remote repository to the current directory,
    /// and initialize a new Simperby node after verification.
    Clone {
        /// The URL of the remote repository.
        url: String,
    },
    /// Sync the `finalized` branch to the `work` branch.
    ///
    /// This will verify every commit along the way.
    /// If the `work` branch is not a descendant of the
    /// current `finalized` (i.e., cannot be fast-forwarded), it fails.
    ///
    /// Note that you MUST provide `last_finalization_proof` as an argument which verifies
    /// finalization of the last block of the `work` branch (which must also be the last commit)
    /// This is because the finalization proof for a block exists in the next block.
    /// In other words, if your `work` branch contains N blocks, (N-1) preceding blocks are
    /// verified by its (N-1) following block, but the last block must be manually verified.
    Sync {
        #[clap(short, long, action)]
        last_finalization_proof: String,
    },
    /// Print the information about the Git server that this node is hosting.
    Git,
    /// Clean the repository, removing all the outdated (incompatible with `finalized`) commits.
    Clean {
        /// If enabled, it will remove
        /// 1. all branches except `finalized`, `fp`, and `work`
        /// 2. all remote repositories
        /// 3. all orphan commits
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
    /// Show the overall information of the given commit.
    Show { commit: String },
    /// Run the Simperby node indefinitely. This is same as running `serve` while
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
    /// Serve the p2p network indefinitely.
    Serve,
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
    /// THIS IS TEMPORARY.
    GenesisProposer,
    /// THIS IS TEMPORARY
    GenesisNonProposer,
}
