# Simperby: the DAO Spaceship for Multichain Journey

## Introduction

One of the most important features of Simperby is its multichain
interoperability. Any DAO (Decentralized Autonomous Organization) built with
Simperby can interact with various Dapps (Decentralized Applications) deployed
on various existing blockchains, such as Ethereum. This could include a treasury
for the organization, a token that the organization issues, or any other
application that the organization wants to build or use. This multichain
communication is performed in a **trustless and verifiable manner**, with
reasonable cost and efficiency.

## Light Client

A light client is a special optimization of a blockchain node that is capable of
verifying transactions or state entries without having the full blockchain
state. This is also known as Simplified Payment Verification (SPV) in Bitcoin.

The key idea of a light client is that a blockchain is still **cryptographically
meaningful even if we take only the headers** of the chain (note that the block
body contains transactions).

In Tendermint and Vetomint, the light client has a "last trusted header" as its
state, and only accepts a new header if it is actually finalized by the
consensus, as the next block to the last trusted header. **There is no authority
or trust in the header provider** as there is always a cryptographic proof for
the finalization, the pre-commits from over 2/3 of the validators. If both the
content of the header and the accompanying pre-commits are valid, the light
client can update its state to the new header. Note that to verify the
pre-commits, the light client also must know the eligible validator set for the
new block. Fortunately, a block header contains either the validator set itself
(Simperby's way) or the hash of it (most other blockchains do this way because
they produce blocks frequently, so reducing the header size is important), so
verification can be trivially handled.

Now we know that a light client can track the canonical chain of headers only by
constantly accepting the headers and the proofs in a trustless and verifiable
way. Since a block header contains the Merkle root of the state and
transactions, the **light client can verify the inclusion of any state entry or
transaction with its Merkle proof**, at the given height. The light client may
store all previous headers that it has accepted so far to verify the inclusion
from past blocks.

## Trustless Message Delivery

A light client takes blockchain headers and their proofs as the only input to
update its state. In other words, the light client can stay in sync with the
blockchain without accessing the network if there are constant header updates
from whoever submits valid ones.

Thanks to this simple but powerful property, it is possible to embed a light
client as a smart contract. This enables **trustless message delivery between
two different blockchains**, which is the key to multichain interoperability.

Suppose there are two different blockchains called A and B. If the A chain has a
light client of the B Chain deployed on A chain, whatever finalized on the B
chain can be instantly verified on the A chain. Other contracts deployed on the
A chain may interact with the B chain since the light client provides data from
the B chain after verification. This communication can trivially work both ways
if the B chain has a light client of the A chain as well.

Cosmos IBC (Inter-Blockchain Communication) is an example of a protocol based on
this idea. IBC defines multiple layers over the core communication primitive
(two different chains embedding light clients of each other) to establish
high-level communication such as asset transfer. Various chains built with
Cosmos SDK (a.k.a. zones) are all standalone layer-1 networks, but they form an
organic multichain structure using IBC.

## Settlement Chain

Simperby is a standalone blockchain that boasts its own consensus mechanism and
distributed file system. However, there must be working applications that
interact with the Simperby chain so that the organization can finalize something
meaningful. The most important applications in this regard would be other
established blockchain ecosystems, which the organization seeks to interact
with. From Simperby's view, they are called **settlement chains**. Any
blockchain that has a smart contract platform can be a settlement chain once the
organization pays some gas and deploys their light client on it.

Simperby's scalability over settlement chain integration is quite impressive;
the source code for the light client contract will be almost the same for every
chain except for a few virtual machine-specific behaviors and the host
interface. This enables **scalable development of light clients over various
settlement chains**, which will eventually provide powerful multichain
interoperability for the DAO that uses Simperby.

Also, considering that a Simperby chain has no contracts or business logic but
is only capable of recording explicitly approved data, there is nothing to
execute automatically on the Simperby side. That is, the multichain
communication system **doesn't have to be bidrectional**. Things happening
(finalized) in the settlement chain might be delivered to the Simperby chain in
the same way (embedding a light client) but that's just pointless; there's
nothing to programmatically respond to such events because Simperby does not
host a contract platform. Instead, members of the Simperby chain will manually
check the result from the settlement chain using whatever method they want and
take it into account if it matters in making a decision on the next agenda. They
could use a standalone light client, some centralized explorer like Etherscan,
or some could even ignore the result if they think it's not important for the
next agenda.

There is **no communication between settlement chains** too. The number of
required communication channels (i.e., number of light clients) are the same as
the number of settlement chains that the organization deploys on.

### Currently Supported Settlement Chains

#### EVM

TODO

#### Non-EVM

TODO

## Dapps

Using trustless message delivery, DAOs using Simperby can have various
multichain applications. One of the most important applications is a
**treasury** which holds tokens since managing shared assets is by far the
baseline for establishing any working organization.

Thus, for every supported chain, both the light client and treasury contracts
are essential; Simperby provides them in a single contract while exposing an
interface to request verification (by the light client) of the incoming
messages, for other contracts.

Here are other possible use cases of Simperby-controlled contracts:

1. A token that the organization issues and manages.
2. A complex version of the treasury that might interact with other DeFi
   services.
3. A delegator to participate in other on-chain DAOs, on behalf of the Simperby
   DAO.
4. A DeFi pool whose parameters are controlled by the Simperby DAO.
5. A bridge between two different chains, where the Simperby DAO acts as the
   bridge provider.

Simperby DAOs can also interact with external Dapps. They can create a proxy
contract to serve as a user of a specific Dapp. Additionally, as the Simperby
protocol becomes mature and common, there will be an increase in the number of
Dapps that have built-in support for Simperby DAOs.

## Universal Interoperability

DAO is a technology to build an organization over decentralized networks, but
its subject is not just limited to blockchains. The Simperby protocol could
govern the real-world parts too, because we already have a solid way to deliver
what's finalized by the consensus. This simple and trustless light client can be
easily embedded as a super lightweight module in any application that is
required to interact with a Simperby DAO. It could be a plugin attached to a
centralized service, a webpage, or even a legal contract. There's **no need for
a secure channel or complex authorization mechanism; it just works out of the
box**, in a fully trustless and end-to-end verifiable way.

Most of the existing blockchain protocols have light clients, but they could not
be as simple as Simperby's because they have:

1. Frequent block production (Simperby produces blocks on demand)
2. Too many consensus participants (Simperby is a blockchain engine for
   permissioned organizations)
3. Complex consensus algorithm (Simperby uses a variation of Tendermint, which
   is battle-tested in the Cosmos ecosystem)
4. Non-portable implementation (Simperby uses Rust, which can be both native and
   WASM-compiled)

## DAO Spaceship

Simperby is also known as **the engine for the DAO spaceship.** A DAO built with
Simperby is completely standalone, self-hosted, and fully sovereign. It has zero
dependency on any other blockchain or centralized service. It's like a spaceship
floating over the space, visiting and colonizing various planets (the settlement
chains).

All you need to do is just run a super-lightweight node to maintain the
distributed protocol. If you think it's time to contact an existing settlement
chain, just deploy your light client and explore the ecosystem. If the
settlement chain suddenly collapses or if you want to move to another one, just
deploy another light client and you're good to go. You can even have multiple
settlement chains that you effectively control. No matter what happens in the
settlement chain, your DAO remains intact and fully functional. **Even if most
of the existing mainnets suddenly disappear and the Web3 industry collapses,
Simperby DAOs will survive, floating over the space and seeking the next
journey**.

## Zero-Knowledge Proof

One potential problem of Simperby's multichain communication system is that the
header update cost is proportional to the number of consensus participants of
the Simperby chain. Since Simperby is a permissioned blockchain, the number of
consensus participants is quite limited, but it could still be costly if it is
executed on an expensive chain like the Ethereum mainnet.

To reduce the gas cost, we can use zero-knowledge proofs to compress signatures
in the block header without compromising the security. Using this method, we can
generate a constant-size proof for the header update regardless of the number of
consensus participants.

This technique is not yet implemented in Simperby, but our team is actively
researching this topic.
