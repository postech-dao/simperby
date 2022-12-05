# Simperby

A simple, permissioned, and BFT blockchain for decentralized organizations.

## TL;DR

Simperby is a blockchain engine that provides

1. **Consensus**: a safe and live mechanism to finalize the state of the organization, which is tolerant to byzantine faults.
2. **Governance**: a democratic mechanism to make decisions on the organization, which can be finalized by the consensus.
3. **Communication Channel**: a mechanism to communicate with each other in a decentralized, verifiable, fault-tolerant way.
4. **Git Repository**: a distributed Git repository that every member can fetch and verify, and also can push commits with governance approval.

Also, Simperby is provided as an **extremely lightweight CLI** software.
It *does NOT run in the background*. All the operations except `simperby run` and `simperby serve` are done *synchronously and explicitly* (i.e. performs the given operation and returns the result immediately).

## DAO

Simperby is the best solution for **decentralized organizations** which is required to be

1. Completely Standalone, Sovereign and Self-hosted
2. Governed by Permissioned Members
3. Decentralized and Distributed
4. Safe and Fault Tolerant
5. Verifiable and Transparent

If you build a chain with Simperby, it instantly becomes **a DAO**, as it functions as a

1. Standalone P2P Communication Channel
2. Governance Platform
3. Solid Consensus and Finality Machine
4. Multichain Interoperability
5. Immutable, Verifiable, and Distributed Storage as a Git Repository

## Governance & Communication Channel & Consensus

See [docs](docs/protocol_overview.md)

## Blockchain as a Git Repository

See [docs](docs/git.md)

## Multichain DAO

See [docs](docs/multichain_dao.md)

## License

This project is licensed under the [MIT license](./LICENSE).

### Contribution

See [DEV-GUIDE.md](./DEV-GUIDE.md).

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in Simperby by you, shall be licensed as MIT, without any additional terms or conditions.

