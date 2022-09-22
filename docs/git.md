# Simperby: the Git-based Blockchain

One of the most unique features of Simperby is its use of Git.

## Pre-requisites

You should have a basic understanding of

1. Blockchain
2. [Simperby Consensus and Governance](./protocol_overview.md)
3. Git

## Summary

1. **Every finalized data in a Simperby chain is stored in a Git repository.**
2. It includes transactions, agendas, agenda proofs, chats, and blocks, linearly committed in the `main` branch.
3. A Simperby node manages its own Git repository and provides a Git server of it to the node operator.
4. The node operator can fetch the blockchain data, walk through the history, and check the diffs using the Git protocol.
5. **Every to-be-finalized data is also managed in a Git repository.**
6. All the pending agendas (waiting for approval) will be presented as multiple branches grown from the `main` branch.
7. The node operator may create their own transaction as a commit and push to a particular branch which represents an agenda proposal.
8. **Simperby funtions as a general distributed Git repository**, that may contain any useful data for the organization. This is trivially achieved because **we take transactions as Git commits.** (This can be understood as exploiting the 'blockchain state' as a Git repository)

## Lifecycle of a Simperby Chain

TODO

### Step 0: finalized block

Let's assume that there is the last finalized block with the height of `H`.
We will take that has a starting point of our recursive process. Of course the base case of the finalized block would be the genesis block.

### Step 1: Agenda

TODO

## Specification

Here we present the specification of the Simperby Git repository.

### Commits

A commit is defined as either

1. `initial`: an empty, initial commit as the very first commit of the repository.
2. `genesis`: a commit that contains the initial state and the genesis info.
3. `block`: a finalized block
4. `tx`: a transaction of an arbitrary update on the state. Note that a `tx` commit is the only exception that the commit title does not start with its type, `tx`.
5. `tx-delegate`, `tx-undelegate`: an extra-agenda transaction that updates the delegation state.
6. `tx-report`: a transaction that reports a misbehavior of a validator with the cryptographic proof.
7. `chat`: chat logs for the height
8. `agenda-proof`: a proof of the governance approval of an agenda

### Commit Format

TODO

### Tags

1. `vote-<hash>`: for agenda commits only; denotes that the node has voted for the agenda.
2. `veto-<hash>`: for block commits only; denotes that the node has vetoed the block.
3. `proposal-<hash>`: for block commits only; denotes that the node has decided the block as the proposal.

### Structure

```text
// The history grows from the bottom to the top.
// Each line represents a Git commit.

block H+1 (tag: finalized)
chat H+1
[extra-agenda transactions]
...
agenda proof H+1
agenda H+1
[ordinary transactions]
...
block H
```

If the node receives multiple agendas, it presents multiple branches that consist of `ordinary transactions` and a single `agenda` grwon from `block`.

### Example

If an organization using Simperby keeps their repository public, it is natural to have a mirror of the block data repository on a publicly hosted service like Github.

We present an example of the block data [here](https://github.com/postech-dao/simperby-git-example)
