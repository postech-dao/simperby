use clap::{Parser, Subcommand};
use simperby_core::MemberName;
use simperby_node::simperby_core::BlockHeight;

/**
Welcome to the Simperby CLI!
*/
#[derive(Debug, Parser)]
#[clap(name = "simperby")]
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
        chain_name: String,
    },
    /// An extra-agenda transaction that undelegates the consensus voting power and
    /// the governance voting power (if delegated).
    TxUndelegate {
        delegator: String,
        block_height: BlockHeight,
        proof: String,
        chain_name: String,
    },
    /// An extra-agenda transaction that reports a misbehaving validator.
    TxReport, // TODO
    /// A block waiting for finalization.
    Block,
    /// An agenda waiting for governance approval.
    Agenda,
}

#[derive(Debug, Subcommand)]
pub enum PeerCommand {
    /// Add a peer with the given name and address.
    Add { address: String, name: String },
    /// Remove the peer with the given name.
    Remove { name: String },
    /// Updates the peer list using the peer discovery protocol.
    /// This may leave some remote repositories with the prefix `>`.
    Update,
    /// Prints the status of the discovered peers.
    Status,
}

#[derive(Debug, Subcommand)]
pub enum SignCommands {
    TxDelegate {
        delegator: MemberName,
        delegatee: MemberName,
        /// Whether to delegate the governance voting power too.
        governance: bool,
        target_height: u64,
        chain_name: String,
    },
    TxUndelegate {
        delegator: MemberName,
        target_height: u64,
        chain_name: String,
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
    ///
    /// This is same as `git clone && cd <directory> && simperby init`.
    Clone {
        /// The URL of the remote repository.
        url: String,
    },

    // ----- Modification Commands ----- //
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
    Consensus {
        /// If enabled, it shows the status of the consensus instead of making a progress.
        ///
        /// Unlike the governance which is performed on each agenda,
        /// the consensus is 'global' so this option is not associated with any commit.
        #[clap(long, action)]
        show: bool,
    },

    // ----- Information Commands ----- //
    /// Show the overall information of the given commit.
    Show { revision: String },
    /// Show the status of the Simperby repository.
    ///
    /// It checkes the following for the current directory:
    ///
    /// 1. Is it a valid Git repository?
    /// 2. Does it contain a valid `.simperby/` directory?
    /// 3. Does it have a valid `reserved/` directory and `.gitignore`?
    /// 4. Does it have all the protected branches and tags?
    /// 5. Does the reserved state at `finalized` branch match the block header?
    /// 6. What phase is the `work` branch in?
    /// 7. Does the `fp` branch match the last block header?
    /// 8. Isn't your repository behind the latest consensus status? (finalized but not yet received the actual commits)
    Status {
        /// If enabled, it performs a full verification of the entire history
        /// of the chain, starting from the genesis commit.
        #[clap(long, action)]
        full: bool,
    },

    // ----- Network Commands ----- //
    /// Show the current status of the p2p network.
    Network,
    /// Manages the peer list for the p2p network.
    /// Note that this is independent from the Git remotes.
    #[command(subcommand)]
    Peer(PeerCommand),
    /// Become a server node indefinitely, serving all message propagations and Git requests.
    /// This is same as regularly running the `update` and `broadcast` commands.
    ///
    /// You cannot perform any other operations while running this command;
    /// you have to run another shell to perform client-side, synchronous operations.
    Serve,
    /// Update the node state by fetching data from the p2p network,
    /// verifying incoming data, and applying to the repository and consensus & governance status.
    Update {
        /// If enabled, it performs only local updates without fetching data from the network.
        ///
        /// "Local updates" means changes (new commits) that have been manually created or git-fetched by the user.
        #[clap(long, action)]
        no_network: bool,
    },
    /// Broadcast relevant data to the p2p network.
    ///
    /// This may contain
    ///
    /// 1. Newly created commits
    /// 2. Consensus progress
    /// 3. Governance votes
    /// 5. Known peers
    /// 6. All of the above, sent from other peers.
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
