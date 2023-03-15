# Modules

This guide is intended for developers and provides an overview of the major
modules within the Simperby project.

The project consists of 4 major modules:

1. Repository: This module manages the local repository of a node, which tracks
   the canonical chain of the network.
2. Governance: This module manages a node's client in the p2p governance voting
   protocol.
3. Consensus: This module manages a node's state machine of the BFT algorithm
   (Vetomint).
4. Chat: This module manages a node's client to the p2p chat protocol.

Each module interacts with the network through an abstracted interface to the
p2p network called `DistributedMessageSet` (DMS), which provides access to
broadcasted messages over the network.

## Common Process

- Each module is intialized with an instance of `DMS`.
- During each operation, the module may store some messages to its local DMS.
- During the `flush()` operation, the module stores all the messages it wants to
  broadcast. These messages may or may not have been stored by other operations.
  The module may also have another storage (other than DMS) to store its
  internal states, which can deduce the set of messages to be broadcasted upon
  the `flush`() operation.
- In the `update()` operation, the module reads the message stored in DMS and
  updates its internal states accordingly. These messages may or may not have
  been updated by other operations.
- The modules don't have direct access to broadcast or fetch messages from the
  network. Instead, the node (which manages all the modules) will handle the
  broadcasting and fetching of messages.
- Before and after broadcasting and fetching the messages from the network, the
  node will call the `flush()` and `update()` functions of each module.

## Repository

TODO

## Governance

TODO

## Consensus

TODO

## Chat

TODO
