# Simperby: the Git-based Blockchain

One of the most unique features of Simperby is its use of Git.

## Pre-requisites

You should have a basic understanding of

1. Blockchain
2. [Simperby Governance and Consensus](./protocol_overview.md)
3. Git

## Summary

1. **Every finalized and to-be-finalized data in a Simperby chain are stored in
  Git repositories.**
2. All the finalized data, including transactions, agendas, agenda proofs,
  chats, and blocks, are committed linearly to the `finalized` branch.
3. All the to-be-finalized data, including pending agendas (waiting for
  approval) and blocks (waiting for finalization) are presented as multiple
  branches grown from the `finalized` branch.
4. Each Simperby node manages its own Git repository and provides a read-only
  Git server to peers and the node operator.
5. The node operator can `git fetch` the local server that the Simperby node
  manages to their own local clone, **walk through the history, and check the
  snapshots and diffs using whatever Git client**.
6. **Peers** can also **fetch blockchain data from each other, verify commits,
  and synchronize** the blockchain state or pending proposals.
7. The node operator may create their own transactions as commits and push them
  to a designated branch (`work`) to create **a proposal to rebase on the
  `finalized` branch**.
8. **Simperby functions as a general distributed Git repository**, that can
  contain any useful data for the organization. This is trivially achieved
  because **Simperby takes Git commits as blockchain transactions.** Any
  commits made on the canonical branch (`finalized`) of the repository will be
  stored permanently, in a Byzantine fault tolerant and distributed way.

### Correspondence

The following holds for correspondence:

1. Transactions and block headers: commits
2. The canonical history of the blockchain: the `finalized` branch
3. A block proposal is finalized: rebased on the `finalized` branch
4. State snapshot: work tree of the repository
5. Start a new node: clone the repository
6. Peer-to-peer networking: fetch the repository from each other

## Why Git?

Git is a distributed version control system that is widely used in the software
development community. It is a mature technology that has been used for decades.
There are four main reasons why we use Git as the underlying storage of Simperby.

### Version Control System

Git is a version control system, first of all.
It provides wonderful features like branching, merging, rebasing, and diffing.
You can checkout to any commit and browse the file system at that point in time.
It manages the entire history of the repository in a linear or DAG structure.

This fits amazingly well with Simperby, because.. TODO

### File System

Git manages a file system that can be directly browsed by the users via the host
operating system. As Simperby is a blockchain for an organization, not a
contract platform, the role of the blockchain state is to serve as a
general-purpose data storage. Here are a few examples of the use of a
distributed file system for an organization:

1. Replacement of a shared file system like [Google Drive](https://www.google.com/drive/)
2. Storage for a static file server for the website that the organization owns.
3. Codebase of the organization - also the most common use case of Git itself
4. Diff-sensitive data like the law or the constitution of the organization
  (articles of association, bylaws, etc.)

If you want to explore the snapshot of the blockchain state (including the past
ones), you can simply checkout to the revision and browse the file system.

### Distributed

Git inherently works in a distributed manner.
A Git repository may have multiple 'remotes' that can be freely added, removed,
and fetched from. There is no 'central server' for the repository.
Based on this principle, in Simperby, each node will add the peers that they
discovered, and may fetch relevant remote branches, verify them and update their
local repository. It could advance the block height by verifying the incoming
block headers moving the local `finalized` branch. It could also track newly
observed agenda or block proposals provided as peer's remote branches,
reflecting on the local repository marked by its own branch.

### Powerful Third-Party Tools and Services

Git is the most used version control system currently.
It has a huge ecosystem of third-party tools and services.

1. Hosting services like [GitHub](https://github.com) or
  [GitLab](https://gitlab.com) can easily **mirror** the repository of the
  organization. This is useful for an organization that wants to provide their
  data to the public. It will save the cost of developing its own block explorer
  and indexing service! It is also possible to add a CI plugin to verify the
  incoming commits as Simperby node does.
2. Clients like [GitKraken](https://www.gitkraken.com/),
  [SourceTree](https://www.sourcetreeapp.com/),
  [GitHub Desktop](https://desktop.github.com/) or various extensions on your
  text editor will make both exploring and editing (for proposers) of the
  repository much easier and more productive.

## Specification

Here we present the specification of the Simperby Git repository.

### Commits

A commit is defined as follows

1. `block`: an empty commit for the either proposed or finalized block
2. `tx`: a transaction of an arbitrary update on the state (except the reserved
  directory). Note that a `tx` commit is the only exception that the commit
  title does not start with its type, `tx`. It may be empty.
3. `tx-delegate`, `tx-undelegate`: a non-empty extra-agenda transaction that
  updates the delegation state which resides in the reserved directory of the repository.
4. `tx-report`: a non-empty commit that reports the misbehavior of a validator
  with cryptographic proof. This must include the state change caused by the slashing.
5. `chat`: an empty commit for the chat logs of the height.
6. `agenda-proof`: an empty commit for the proof of the governance approval of
  an agenda.

### Commit Format

TODO

### Branches

These are the names of the branches that are specially treated by the Simperby
node. Branches other than `work` and `p` are managed by the node; it will be
rejected if pushed.

1. `finalized`: always points to the last finalized block.
  It is strongly protected; users can't push to this branch.
2. `work`: the only branch that users can freely push or force-push.
  CLI commands like `create` interact with this.
3. `p`: the block proposal for this node. The node operator may push or
  force-push to this branch. When pushed, the Git server will check the validity
  of the branch. The consensus engine will recognize this branch and propose to
  the consensus. It stands for 'block proposal'.
4. `a-<hash>`: a valid agenda (but not yet approved) propagated from other
  nodes. If the governance has approved the agenda, it will point to the
  `agenda-proof` commit which lies on top of the agenda commit. The `<hash>`
  MUST be the hash of the commit, truncated in the first 8 digits.
5. `b-<hash>`: a valid (but not yet finalized) block propagated from other
  nodes. The `<hash>` MUST be the hash of the commit, truncated in the first 8 digits.
6. `fp`: a very special branch that always holds the finalization proof for the
  last block. This is required because a block header doesn't contain the
  finalization proof of itself. Thus, to make a repository self-verifiable, it
  is essential to have the proof somewhere, in some way. This branch has only
  one empty commit that is directly on top of the corresponding block commit,
  titled with `fp: <height>`. The commit message body contains the actual proof.
  **Note that the commit of `fp` branch could differ between nodes due to the
  different observations of the signers**, but the proof itself must be valid.

### Tags

Tags can't be pushed by the users. They are always managed by the nodes.

1. `vote-<number>`: for agenda commits only; denotes that the user has voted for
  the agenda. The `<number>` is assigned arbitrarily by the node.
2. `veto-<number>`: for block commits only; denotes that the user has vetoed the
  block. The `<number>` is assigned arbitrarily by the node.
3. `genesis`: the genesis block.

### Structure

```text
// The history grows from the bottom to the top.
// Each line represents a Git commit.

block H+1 (branch: finalized)
chat H+1
[extra-agenda transactions]
...
agenda proof H+1
agenda H+1
[ordinary transactions]
...
block H
```

If the node receives multiple agendas, it presents multiple branches that
consist of `ordinary transactions` and a single `agenda` grown from `block`.

### Genesis

In Simperby, blockchain genesis is defined as creating a first `block` commit on
an existing Git repository. The repository can be in any state, and all the
history will be preserved as a 'pre-genesis' era. The **Git commit hash** (not
the Simperby hash) of the last commit in the pre-genesis era (the parent of the
genesis block) will be contributed to the `previous_hash` field of the genesis
block header.

### Example

TODO
