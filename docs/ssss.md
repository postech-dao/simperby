# Simperby Super Simple Summary

## DAO

A DAO (Decentralized Autonomous Organization) is an organization that operates
autonomously on a blockchain without relying on government or centralized
control. It is governed by a set of rules encoded in the protocol, and is based
on a trustless network where all decisions are recorded and verified by the
network participants.

For more information on DAOs, refer to [the introduction to DAO
(WIP)](./dao.md).

## Limitations of DAOs reliant on existing blockchains

The most common way to implement a DAO is by **using an existing blockchain that
supports smart contracts**. This involves programming the governance rules of
the DAO into a smart contract and deploying it on the blockchain. However, this
approach **has some disadvantages**:

1. Users need to pay **gas fees** to operate the DAO.
2. It is reliant on the performance, security, reliability, usability, and
   potential political risks **associated with the chosen blockchain**.
3. Its capabilities are restricted by the ecosystem of the chosen blockchain and
   may **not easily interact with other blockchains**.

Simperby protocol solves these problems by providing a standalone blockchain for
a DAO.

For more details, refer to [the comparisons (WIP)](./comparison.md)

## Simperby

Simperby is a blockchain engine that builds a standalone blockchain for a DAO
with the following features:

- Self-hosted and standalone
- Trustless
- Distributed
- Fault Tolerant
- Lightweight
- Highly Interoperable
- Native Support for Multichain Treasuries

For details on the core protocol, refer to the [protocol
overview](./protocol_overview.md).

### Byzantine Fault Tolerant System

- **Builds an independent layer-1 blockchain** like [Cosmos
  SDK](https://v1.cosmos.network/sdk) or [Substrate](https://substrate.io/).
- **One blockchain for one DAO**.
- **Self-hosted: DAO members maintain the chain**.
- Has own **BFT consensus algorithm** called [Vetomint](./vetomint.md) optimized
  for sporadic node operations.

### Governance and Membership

- A peer-to-peer voting mechanism is used to make decisions on the organization,
which is then finalized by the consensus.
- No tokens, no staking, no mining, no gas fee; instead, **permissioned and
explicitly nominated nodes**.
- Has its own peer-to-peer communication channel for the organization.

### Storage

- Provides a distributed file system for the organization.
- **Managed as a distributed Git repository**.
  - Blocks and transactions are stored as Git commits.
  - Governance and consensus proposals are stored as Git branches.
  - The canonical history of the chain is presented as a designated Git branch.

For details on the storage, refer to the [documentation](./git.md).

### Interoperability

- Native support for trustless message delivery that establishes **multichain
  interoperability** directly.
- **Based on a light client**, which verifies the finality of the chain without
  requiring the full state or full block, sharing the same principle as [Cosmos
  IBC](https://ibc.cosmos.network/).
- Light clients will be uploaded as a contract on the existing chains that
  organizations want to interact with, known as the **settlement chain**.
- These light clients verify the instructions made by the organizations on the
  Simperby chain and execute them on the settlement chain.
- Easy implementation over various existing blockchains, thanks to its simple
  and lightweight architecture.
- Effective control of treasuries, tokens, and other dapps owned by the
  organization without requiring a trusted third party.
- **Treasury contracts** for the supported blockchains are available out of the
  box.

For details on this protocol, refer to the [documentation](./multichain.md).

### Sporadic & Lightweight Node Operation

- Simperby's novel consensus protocol allows node operations to be sporadic;
  **nodes are not required to operate 24/7**. Participants may turn on their
  nodes only when there is a proposal to vote on.
- **Blocks are produced on demand**, only when there is a governance-approved
  transaction.
- Each Simperby node runs a **lightweight CLI software** that **does not run in
  the background**.
- All operations are synchronous and explicit.
- Participants can act as client nodes, performing required operations,
  broadcasting messages to the peer-to-peer network if needed, and returning
  results immediately.
- At least one working server node is required to receive broadcasted messages
  but it does not have any authority and is only responsible for relaying
  messages.
- The performance of the overall system is not impacted by this strict node
  uptime condition because Simperby is not a general contract platform that
  needs to store the contract state and process contract-invoking transactions
  with low latency.
