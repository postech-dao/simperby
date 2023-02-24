use clap::{Parser, Subcommand};
use simperby_node::simperby_common::BlockHeight;

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
        block_height: BlockHeight,
        proof: String,
    },
    /// An extra-agenda transaction that undelegates the consensus voting power and
    /// the governance voting power (if delegated).
    TxUndelegate {
        delegator: String,
        block_height: BlockHeight,
        proof: String,
    },
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
    // ----- Initialization Commands ----- //
    /// Create a genesis commit and initialize a Simberby repository
    /// from the given existing Git repository.
    ///
    /// This will seek the reserved state, verify it, and add a genesis commit.
    /// You have to run `init` after this to initialize the Simperby node.
    Genesis,
    /// Initialize a new Simperby node from the given existing Simperby repository.
    Init,
    /// Clone a remote Simperby repository to the current directory,
    /// and initialize a new Simperby node after verification.
    Clone {
        /// The URL of the remote repository.
        url: String,
    },

    // ----- Modification Commands ----- //
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
    /// It will also leave a `vote` tag on the given commit (with some postfix).
    Vote { revision: String },
    /// Veto the round.
    ///
    /// It will be broadcasted to the network as a nil-vote
    /// in the next `consensus`, if the conditions are met.
    /// You can check whether the round is vetoed by running `consensus --show`.
    Veto {
        /// If specified, it vetoes a specific block proposal,
        /// leaving a `veto` tag on the given commit (with some postfix).
        /// It fails if the commit is already set to `proposal`.
        /// If the commit is already set to `veto`, it will be removed.
        revision: Option<String>,
    },
    /// Make a progress on the consensus.
    ///
    /// The node may broadcast the proposal or consensus messages depending on the
    /// current consensus round state.
    Consensus {
        /// If enabled, it shows the status of the consensus instead of making a progress.
        ///
        /// Unlike the governance which is performed on each agenda,
        /// the consensus is 'global' so this option is not associated with any commit.
        #[clap(long, action)]
        show: bool,
    },

    // ----- Information Commands ----- //
    /// Print the information about the Git server that this node is hosting.
    Git,
    /// Show the overall information of the given commit.
    Show { revision: String },
    /// Show the current status of the p2p network.
    Network,

    // ----- Network Commands ----- //
    /// Become a server node indefinitely, serving all message propagations and Git requests.
    ///
    /// You cannot perform any other operations while running this command;
    /// you have to run another shell to perform client-side, synchronous operations.
    Serve,
    /// Update the node state by fetching data from the p2p network,
    /// verifying incoming data, and applying to the repository and consensus & governance status.
    Update,
    /// Broadcast relevant data to the p2p network.
    Broadcast,

    // ----- Miscellaneous Commands ----- //
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
    /// A special command triggered by the Git hook, which is used to verify the push request.
    CheckPush {
        /// The revision of the branch that is being pushed.
        revision: String,
        /// The name of the branch that is being pushed.
        branch_name: String,
        /// The Unix timestamp of the push request. This is for preventing replay attacks.
        timestamp: u64,
        /// The signature by the pusher.
        signature: String,
    },
    /// A special command triggered by the Git hook, which is used to notify the push request.
    NotifyPush { commit: String },
}
