# Simperby: Protocol Overview

Simperby is a blockchain engine that runs an organiziation which is

1. **Decentralized**
2. **Standalone, Sovereign and Self-hosted**
3. **With a set of permissioned members**

in that it provides

1. **Consensus**: a safe and live mechanism to finalize the state of the organization, which is tolerant to byzantine faults in an asynchronous network.
2. **Governance**: a democratic mechanism to make decisions on the organization, which can be finalized by the consensus.
3. **Communication Channel**: a mechanism to communicate with each other in a decentralized, verifiable, and fault-tolerant way.

## Basics and Terminologies

The reader of this document should be familiar with the basic concepts of blockchain.

- Simperby is an engine that can build an instance of a blockchain.
  Any blockchain built with Simperby is sometimes called a *Simperby chain*.
- The unit of state transition is called a *transaction*.
  `StoreData`, `DelegateGovernanceVotingPower`, `AddValidator` are the examples.
- A sequence of transactions is called an *agenda*.
  **Any governance voting is performed on an agenda and the target block height**.
- A *block* consists of the header and **a single agenda, which is mandatory**.
- The participants of the governance are called *members*.
- The participants of the consensus are called *validators*, which is a subset of the *members*.
- A *consensus leader* or a *block proposer* is a validator that proposes a block for the round.
- A *round* is a period of time in which a consensus leader proposes a block, and consensus votes(pre-vote, pre-commit) are performed. Refer to Tendermint for more details.
- A validator is *honest* or *not byzantine* if they follow the consensus protocol.
- A block proposer is *faithful* if they fulfill their responsibility and *good* if they don't overuse their power. They are *lazy* and *bad* otherwise.
  The notion of *responsibility* and *power* will be explained [later](#consensus-leader).

## Summary

Here is a summarized version of the Simperby protocol.

1. We want the node operation to be **simple and lightweight** to realize a truly distributed, decentralized and self-hosted organization.
2. To be so, the key assumption is that most of the nodes are **rarely online** and the protocol produces new blocks **on-demand**.
3. To accomplish that, the **consensus round must be very long**.
4. For every block, members can **vote on or propose an agenda, propagating** to each other.
5. At the same time, members can also **propagate their chats** used for the communication, which is **coordinated by the block proposer**.
6. If there is a **governance-approved (majority-voted) agenda** on the network, the block proposer should **include it in the block along with the chat log**, and propose to the consensus.
7. In that the block proposer should be online most of the time (chat coordination and block proposing), this **responsibility should be laid on a few of the validators most of the time**, to ensure the rarely-online assumption.
8. Also the role of **block proposer has some authorities** (chat coordination and agenda inclusion) that can't be cryptographically verified if misused (typically censorship).
9. Thus there exists **'few of the validators' that can be lazy or commit harmful misuses** of their authorities which aren't really byzantine faults or invalid blocks.
10. Normally this isn't a problem, if the rounds are short and the block proposer changes regularly, but we're not.
11. We need a special mechanism to **'veto' the block proposer not to waste long rounds in the consensus layer**. Thus we introduce a special variation of Tendermint called *Vetomint*.
12. In Vetomint, validators can veto the current block proposer, but still, the round will progress (changing the block proposer) if all the honest validators either vote or veto.

## Simple and Light Node Operation

Simperby strongly encourages each member to run their node,
to ensure that the network is decentralized and distributed (and so truly self-hosted). In other words, running a Simperby chain is not about using AWS servers by a few people but using their laptops to physically run the chain. To achieve that, it is important to **keep the node operation as simple and lightweight as possible**.

### Rarely-online nodes

One of the most important conditions for 'simple' and 'lightweight' node operations is how often the node needs to be online.
If we want to make the validators run nodes on their laptops, it is not realistic to assume that they will be online 24/7.

Since Simperby's BFT consensus assumes a partially-synchronous network, rarely-online nodes can be trivially handled because it's no more than one typical case of an asynchrony, if

1. the consensus round is (or has grown to be) long enough to cover all appearances of the nodes.
2. at least one node is online when there is a network broadcast

Condition 1 turns out to be a tough challenge in the later sections, so keep it in mind.

### On-demand block production

Rarely-online nodes mean rarely-progressed blockchain.
(note that in any BFT algorithm, at least 2/3 of nodes must have participated in the consensus process to produce a block)

This will inevitably compromise the latency or throughput of the blockchain, but it doesn't matter because
**Simperby is NOT a general contract platform for serving public users,**
**but a governance platform for a set of permissioned members**.

Simperby blockchain includes only governance-approved state transitions (there are very [few exceptions](#consensus-leader)).
Every governance agenda must be approved by a majority vote of the members so it will take time(days or even weeks).
Considering that, naturally, Simperby's performance is bound by the governance process, not the consensus which is slowed by lightweight node operation.

### Multichain DAO

One of the notable use cases of Simperby is that it can establish a **muli-chain DAO**. (See [this](./multichain_dao.md) for more details)
It's super-easy to expand the organization to other existing chains (so-called 'colony chains') using
the light client of the Simperby consensus.
Since the light client is uploaded as a contract to other chains,
any block progress of the Simperby consensus will require an additional
transaction to update and store the Merkle root of it.
This might not be cheap, because **the organization will have to pay for the gas cost for every colony chain, for every Simperby block**.
That is another reason that on-demand block production is considered reasonable.

## Governance

Simperby's governance is implemented by P2P voting.

- Any member can propose an agenda, and it will be propagated over the P2P network.
- Any member also can cast a vote on the agenda if they prefer it. The vote will be propagated as well.
- Any agenda that has over 50% votes is **eligible** to be included in a block.
- A block can be **valid only if it includes an eligible agenda**. In other words, empty blocks are not allowed.
- To decide which eligible agenda to finalize is the role of consensus, not the governance.
- The consensus is a BFT algorithm based on a leader-and-round style.

### What can be an agenda?

- Transaction is only the unit of a **state transition**, not an individual item signed and broadcasted as in the case of a conventional blockchain.
- An 'agenda' is defined as `(Proposer, Height, [Transaction])`.
- Only a **single agenda** can be included in a block. This is for preventing complicated dependency problems.
  - If multiple agendas are allowed, each agenda must be independent; Otherwise, voters can't be sure how each agenda is ordered, or even whether it is included in the finalized block. This will cause a severe restriction of the possible agenda items.
- Because `Height` is a part of an agenda,
  every agenda and its vote will be outdated and thus discarded if the block height progresses.

## Chatting

Again, Simperby is a blockchain engine that runs a **standalone, sovereign, and self-hosted** organization.
Thus, the communication channel for the organization should be so as well.
The only way to implement such channel (instead of Discord) would be to leverage the existing P2P network.

- Any member can broadcast its message, signed by their public key.
- Each message contains a list of hashes of every previous messages that the signer has perceived, **forming a chain**.
- Like PoW, the longest chat chain is considered a candidate for the canonical one.
- The consensus leader (block proposer) for the round plays a special role.
  - the leader can broadcast a chat as well, but such chat is considered a **semifinalized point**.
  - A *faithful* leader would **frequently** insert its `ack` chat **on the (seemingly) longest chain**.
  - A *good* leader must not commit a fork (semifinalizing two conflicting chains)
- On top of the recent leader-semifinalized chat, other members continue chatting using the longest-chain rule until the next finalization point.
- Once the governance reaches an eligible agenda, the leader must semifinalize the chat chain for the last time, and include it in the block. If the block is finalized by the consensus then the chat log for the block is finalized as well.
- A *good* leader must include the last-semifinalized chat chain which is the one that the other members would consider the canonical one.

### Why doesn't the leader just serve a chat server?

The reason that the leader only 'semifinalizes' the chat chain, instead of running a full chat server, is that it is more censorship-resistant.

- Compared to a full chat server, the longest-chain protocol has a
  **weak** (because of the lack of network synchrony) **consensus** (longest chain) of the canonical total chat ordering, among the members.
- Thus if the leader attempts censorship (i.e., repeatedly semifinalizes a chat chain that is not the longest one observed by other members), it's easy to *notice* that, though it's not theoretically verifiable due to the assumption of network asynchrony. (it might be a coincidence originated from the possible network delay around the leader)
- Nonetheless, if the leader attempts censorship more, it becomes more suspicious.

Note that if there is a malicious member who tries to spam the chat, the leader will not finalize the spam chain, and the other members would recognize such censorship as something `not bad`.

### What happens in the consensus?

The consensus leader (*block proposer*; don't be confused with the *agenda proposer*) includes an eligible agenda (if there were multiple eligible agendas, the leader just chooses one) in the block, and proposes it.
**A valid block MUST contain an eligible agenda.**. In other words, if there is no eligible agenda, there is no block progress in the blockchain.

In addition to the agenda, the leader can also include some other transactions.

1. RecordChatLog (chatting logs for the very height): this doesn't change the state, but is used as the governance minutes for the agenda.
2. ReportMisbehavior (double votes in the consensus and chat finalization misbehaviors by a past consensus leader): this will automatically lead to the slashing of the voting power of the misbehaving validator.

These two transactions are the only exceptions that are not part of the agenda.

## Consensus Leader

You might notice that the role of the consensus leader is very important in the simperby governance.

### Responsibilities

The leader should be **responsible**.

- The leader should be online for the whole round, providing enough frequency of chat chain semifinalizations.
- The leader should propose a block as soon as possible if there is any eligible agenda.
- The leader should report misbehaviors as well.

Again, one of the key requirements of Simperby is making the node operation as lightweight as possible, especially in terms of how many times and how long the node should be online. Thus laying the burden of being a leader on a round robin, which eventually involves all the validators evenly, will just exhaust them.

To mitigate this problem, Simperby supports a **stable leader** feature.

- The leader order is determined in the state.
- The leader order doesn't change throughout the heights if the order specified in the state stays the same.
(i.e., the leader for the first round stays the same)
- For a single height, it is also possible to repeat the first few leaders for several rounds. (e.g., `1->1->1->2->2->3->4->...` instead of `1->2->3->4->...`)

Another advantage of a stable leader is that it is **predictable**. Every validator can be quite sure how often they should turn on their node based on the order in the stable leader list.

As long as the selected first few leaders are honest(not byzantine), and *faithful* (fulfilling their responsibilities), the network will be **live**.

### Authority

The leader has **authority**.

- The leader can choose which agenda to include in the block if there are multiple eligible agendas.
- The leader can choose which chat chain to semifinalize.

A *good* node should be objective to agendas; they should choose the first eligible agenda, instead of waiting until it observes another one in their personal favor. Similarly, a 'good' node should be objective to the chat chain semifinalization as well.

As long as the selected first few leaders are honest, faithful, and even good,
the network will be not only **live** but also **fair**. (note that it's always **safe** regardless of the leader's behavior; safety is guaranteed by the BFT consensus)

### So What?

To summarize, the leader has **responsibilities** and **authority**.
Actually, this is not so different from other blockchains that use a leader-based BFT consensus. The leader should be online for the whole round, to receive enough amount of transactions (in the 'mempool'), and the leader chooses which transactions to include in the block. The only difference is that we don't have a user-signed transaction but a governance-signed agenda instead and that we have an additional protocol for the chat finalization.

So how do the 'other blockchains' deal with the leader's responsibilities and authorities? It's simple; they have a very short round time (several seconds),
and rotate the round order every block, so that the actual experience of 'users' (corresponding to 'members' in Simperby) converges to the average of the validator population. In other words, even if some block proposer is lazy (not faithful) or bad (attempting censorship), the users will not severely suffer from it, because it doesn't take long until the next block proposer comes to serve their transactions.

This kind of problem can be characterized as a **single point of failure** problem, that we particularly suffer from because of the long-round and stable-leader conditions.

The only ways to resolve this are:

1. The governance approves an agenda of impeaching the leader, that is, removing it from the validator list. This is an ideal solution because it instantly changes the leader.
2. If the leader doesn't include eligible agendas in their block, there is the last resort: the **governance doesn't approve any agenda**, so that the proposer can't propose any valid block. Remind that an empty block is invalid. This will escape the round after the long round timeout.

## Consensus

So far, we have discussed how the governance and human communication of Simperby work, assuming an abstract underlying BFT consensus. It works well, but we found that there is a serious problem of time lagging in case of laziness. There could be even permanent censorship in case of a bad leader, which can be still resolved after a long time of a round if the governance doesn't approve any agenda. This works, but it's not really efficient considering that 'being long' can be a week or more.

Now we consider the consensus layer as an option for dealing with this, but unfortunately, **there is no way to resolve it in the consensus layer, ideally**. Unlike Byzantine or 'dishonest' behavior which can be directly verified by some cryptographic evidence (e.g. double pre-votes), 'unfaithful' or 'bad' leader can not be handled by the consensus because it's not a violation of the consensus protocol.

### Leader-based vs Leaderless

One might think that a 'leaderless' consensus might be the solution for this. Basically, it might be, but there is a critical problem: network complexity.
In a leader-based consensus, the network can (in the best case) progress if the validators turn their nodes exactly two times (i.e., two times of broadcasts). However, in a leaderless consensus, the number of broadcasts is proportional to the number of validators. This will never be acceptable in our essential assumption of light-weight node operation.

TODO: check it and add reference

### Veto the Round

Again it can't be handled by the consensus layer because neither laziness nor censorship is a violation of the consensus protocol. However, we can find a detour if we **exploit the network asynchrony**. If a validator concludes that the current round should be escaped because of the honest-but-lazy-or-bad leader, they can just **pretend that the round is timed out**. This provides a very special feature over plain Tendermint; **veto the leader**.

Since Tendermint is based on rounds, there is a special mechanism called *nil-vote*. Nodes can cast a nil-vote after the timeout, to escape the current round. This works similarly to the non-nil case, in that it takes two steps.

In plain Tendermint, if the validators use nil-votes to veto the proposer, it just works because it's not so different from a rash timeout, but it might get stuck until the timeout in the split case. (Think about the case of 6/10 non-nil-votes and 3/10 nil-votes, 1/10 Unresponsive Byzantine nodes)
Again, because we're using very long rounds, it will waste a lot of time, waiting for the timeout of the round in that kind of situation. If we encounter this unlucky case of a split, it is no better than the governance's solution (no approval of agendas).

### Vetomint

To improve this, we propose a variation of Tendermint, called *Vetomint*, which is effective in the 'split' case.

### Properties

1. `<1/6` BFT
2. It is allowed (i.e., considered as a non-byzantine behavior) to cast a nil-vote before the timeout, with *some kind of reason*.
3. If all the honest nodes cast either a vote or nil-vote, it is guaranteed that the round progresses; the **early termination**.
4. If all the honest nodes cast a vote, it is guaranteed that the block progresses.
5. If over 2/3 of the nodes cast a vote, the block progresses with the probability of `>0`.

### How does it work?

In Vetomint, unlike Tendermint, the round **always instantly progresses** if over 5/6 nodes either non-nil-vote or nil-vote. Especially when 5/6 nodes vote, it is guaranteed that the height progresses. The threshold 5/6 is just enough to ensure that all nodes observe at least 2/3 (Tendermint majority) of non-nil-votes no matter the individual arrival order of the votes, even with the possible 1/6 nil-votes.
Note that the 'arrival order' matters, unlike Tendermint, because we have a new 'early termination' condition.

Details are explained in issue #4 and the pseudo-code is [here](./vetomint-spec.pdf)

One can easily prove that it is formally safe and live, after some trivial reduction to the original Tendermint.
