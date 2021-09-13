# Overview
Since its inception in late 2012, the Stratum V1 mining protocol has served a vital role in the Bitcoin mining ecosystem. However the mining landscape as changed extensively in the last decade and a major overhaul to the core mining protocol is well overdue. 
Stratum V2 is positioned as the next iteration of Stratum V1, providing major upgrades from an efficiency, security, and decentralization standpoint.

<!-- This section defines a high-level overview of the major design goals of Stratum V2, as well as a summary of the subprotocols and the different roles that use them. -->

# Design Goals
This section defines the high-level design goals of the Stratum V2 protocol.
All design goals are centered around increasing security, decentralization, and efficiency.

1. Preserve the working aspects of Stratum V1 such that Stratum V2 is logically similar wherever possible. This eases the community transition between the two protocols and makes development simpler.
1. Develop a protocol with a precise definition. There is room for interpretation in the Stratum V1 protocol which resulted in slightly different implementations between pools. Stratum V2 is precisely defined such that implementations will remain consistent and compatible.
1. Authenticate the connection between the downstream miner and the upstream Pool Service. Stratum V1 lacks an encrypted connection between the pool and the miner, leaving the miner vulnerable to a variety of Man in the Middle (MitM) attacks. The most common MitM attack being hashrate hijacking, where an attacker intercepts the mining packets with their credentials, routing the hashrate away from the miner and to the attacker. These attack are very real and are difficult for miners to identify, therefore this is imperative to address. Stratum V2 uses an encrypted connection between the pool and the miner, eliminating these threats. The connection model of Stratum V2 is implemented in such a way that the authentication occurs only once, reducing unnecessary packet transmission realized when using the Stratum V1 protocol.
1. All miners to optionally choose the transaction set to mine on. In Stratum V1, transaction selection is sole responsibility of the pool Service
Furthermore, Stratum V2 removes other unnecessary packet 
RRTODO: mining.subscribe and mining.authorize. what other ones? Are these already covered in the above sentence?

Stratum V1 is inherently limiting to the miner in many ways, leaving critical decision making to the pool.
Perhaps the most important decision in mining is the transaction selection performed when constructing the candidate block template.
With Stratum V1, pools are the sole arbiters of this critical process.
This means that only a handful of pool operators have control over which transactions are being mined.

There is currently no mechanism in place to prevent pools from censoring transactions, a direct threat to Bitcoin's decentralization model.
Stratum V2 provides the miner with the *optional* choice to select their own transaction set and build their own Block Template.
Additional infrastructure overhead is required by the miner to take advantage of this feature, but it is vital for the health of the Bitcoin network to have the means in place for miners to select their own transaction set if transaction censorship is suspected by the pool.

Stratum V2 is a flexible protocol intended to move control back into the hands of the miners while increasing both miner revenue and network decentralization.
RRQ: This may not go here?

# Protocol Overview
There are four distinct subprotocols that comprise the full feature set of Stratum V2.
These protocols are the following:

1. Mining Protocol
2. Job Negotiation Protocol
3. Template Distribution Protocol
4. Job Distribution Protocol

There are five possible roles involved when using these subprotocols.
A role here is defined as either a piece of software or hardware, and include the following:

1. Mining Device: The physical hardware performing the hash, typically a Bitcoin ASIC.
2. Pool Service: Produces jobs (for those not negotiating jobs via the Job Negotiation Protocol), validates shares, and ensures blocks found by clients are propagated through the network (though clients which have full block templates MUST also propagate blocks into the Bitcoin P2P network).
3. Mining Proxy: An intermediary node that sits between the Mining Device(s) and the Pool Service that aggregates connections to increase bandwidth efficiency.
The Mining Proxy provides some optional functionality including the ability to monitor the health and performance of the Mining Devices.
(RRQ: What other extra functionality does the Mining Proxy provide? How is the health of a miner evaluated? Just whether it is on, low, off, or disabled? How is the health different from the performance?)
4. Job Negotiator: A node which negotiations with a Pool Service on behalf of the Mining Device(s) to determine which jobs to mine on.
The Job Negotiator receives custom block templates from a Template Provider and negotiates the use of the template with the Pool Service.
It further distributes the jobs to the Mining Proxy (or Proxies) using the Job Distribution Protocol. Often this role is built into the Mining Proxy. (RRQ: Examples scenario of the Mining Proxy encompassing the Job Negotiator and also not?)
This node also communicates with the Template Provider to select the transaction set, then sends the jobs to the Mining Proxies to be distributed the Mining Devices.
5. Template Provider: A Bitcoin node with Stratum V2 compatible RPC commands to allow for transaction selection by the miner. A common example is `bitcoind`.

Figure X depicts the roles and their encompassing protocols.
The furthest upstream component is the pool infrastructure belonging to the Pool Service which encompasses the server-side Mining Protocol and the server-side Job Negotiation Protocol.
Downstream is the mining infrastructure which encompasses the client-side Mining Protocol, client-side Job Negotiation Protocol, Job Distribution Protocol, Template Distribution Protocol, and Mining Devices.
The furthest downstream component are the Mining Devices.
TODO: import figure

## Mining Protocol
This is the core mining protocol and is the direct successor of Stratum V1.
There are several ways to configure the Mining Protocol, from its simplest form where the Pool Service selects the transaction set and communications directly with Mining Device, to a scenario in which a miner selects their own transaction set for thousands of Mining Devices that share single communication channel with the Pool Service. Various scenarios are discussed in the [Functionality subsection](RR TODO: link section) below.

### Communication Channels
In order for the Mining Protocol to support a variety of flexible use cases for the miner, there are three modes of communication, called channels.
These channels define different message formats that are ether passed directly between the downstream Mining Device(s) and the upstream Pool Service, or between the downstream Mining Device(s), the relatively upstream Mining Proxy, and the upstream Pool Service.

1. **Standard Channels**
This is the simplest mode of communication and is the most efficient in terms of bandwidth consumption and CPU load.
To achieve this, some choice is taken away from the miner.
Specifically, the extranonce feature is unused such that the coinbase transaction is not altered and the 32-byte Merkle root hash is sent from the Pool Service to the Mining Protocol node, rather than the Merkle tree branches, reducing bandwidth consumption and CPU load.
RRQ: is Mining Protocol node the right thing to say here?

2. **Extended Channels**
This mode gives extensive control over the search space used by the Mining Device, allowing the miner to implement advanced used cases such as translation between the Stratum V1 and V2 protocols (used if the Pool Service is Stratum V2 compatible but the Mining Device firmware is Stratum V1 compatible), difficulty aggregation (RRQ: what is diff aggregation? Is it similar to a miner choosing their own pdiff?), custom search space splitting (RRQ: is this encompassing the extranonce? what else does this include?), ect. (RRQ: what else is in "etc"?). The use of these features does come at a cost of higher bandwidth consumption and CPU load compared to the Standard Channel, but is still much more efficient than the Stratum V1 protocol.

3. **Group Channels**
This mode is a collection of Standard Channels that are opened within a particular connection so that they are addressable through a communication channel. RRQ: Need more clarity on this channel type. Is this used for a large farm? Why can't Extended Channels be grouped?

## Job Negotiation Protocol
### Motivation
The Job Negotiation Protocol is used when a miner elects to exercise their right to select their own transaction set, a core feature of Stratum V2.
This is in stark contrast to Stratum V1 where only the Pool Service selects the transaction set.
In this way, the pooled mining becomes more akin to solo mining, increasing decentralization.

The ability for a miner to select their own transaction set is vital for the decentralization of Bitcoin.
Currently with Stratum V1, the Pool Service dictates which transactions are included in a block.
This results in a very real threat of transaction censorship by a handful of Pool Services (e.g. pool operators).
The push for decentralization in this context does come at an increase in overhead cost to the miner because the miner must operate and maintain a Block Template Provider (e.g. a `bitcoind` node).

While some miners may still leave the transaction set selection to the Pool Service, it is vital for the mining ecosystem as a whole to have the ability to select their transaction set in case the treat of transaction censorship by the Pool Service does become a reality.
At a minimum, the Job Negotiation Protocol enables a fail safe for miners to fall back on if this unfortunate situation arises.

## Functionality
The Job Negotiation Protocol provides the means for a Pool Service to agree upon a Block Template selected by a Miner (RRQ: or should I say Template Distribution Protocol?). It is very much a negotiation between the two parties as the Miner sends their selected transaction set to the Pool Service who then either accepts the set or rejects it if it is invalid. 
The result of the negotiation can be reused for all mining connections to the Pool Service. Potentially thousands of Mining Devices can share this connection and this negotiated Block Template, creating reducing computational intensity.

## Template Distribution Protocol
### Motivation
The Template Distribution Protocol is used to extract information about the next candidate block from the Block Template Provider (i.e. `bitcoind`), 
replacing the need for `getblocktemplate` (defined in [BIP22](https://github.com/bitcoin/bips/blob/master/bip-0022.mediawiki) and [BIP23](https://github.com/bitcoin/bips/blob/master/bip-0023.mediawiki)).

### Functionality
The Block Template Provider must implement Stratum V2 compatible RPC commands.
RRTODO: What else goes here?

## Job Distribution Protocol
The Job Distribution Protocol is used when miners elect to choose their own Block Template. The Job Distribution Protocol passes the newly negotiated Block Template to interested nodes, complimenting the Job Negotiation Protocol.
Here, a node is either the Mining Proxy (in the case where the Mining Device firmware is only Stratum V1 compatible) or the Mining Device itself (in the case where the Mining Device firmware is Stratum V2 compatible).
In the case where miners elect for the Pool Service to choose the Block Template, jobs are distributed directly from the Pool Service to the interested nodes and the Job Distribution Protocol is not used.

Additionally, it is possible that the Job Negotiation role will be part of a larger Mining Protocol proxy that also distributes jobs, making this sub-protocol unnecessary even when miners do choose their own Block Template. RRQ: I have no idea what this means or if it belongs here.

# Stratum V2 Feature Set
## Binary Protocol
Stratum V2 uses a binary protocol as opposed to the plain-text JSON used in Stratum V1.
Using a machine-readable protocol is much more compact than the human-readable alternative, thereby greatly reducing bandwidth consumption for both the upstream Pool Service and the downstream miner, resulting in lower infrastructure costs for all parties and reducing a miner's stale job rate.

While the core messages remain constant, Stratum V1's nebulous protocol definition resulted in a variety of implementations, all differing between pools.
This can create confusion or potentially result in a fork of the mining protocol, something that is unnecessary and should be avoided for simplicity's sake.
Stratum V2's binary protocol is well-defined, leaving no room for interpretation.

## Encrypted Authentication

## Remove of Redundant Messages
Stratum V1 includes redundant messages left over from legacy mining protocols. Specifically `mining.subscribe` and `mining.authorize`.
RRTODO: Finish

## Header-Only Mining
RRQ: Where and how to include efficient caching? Is efficient caching only done during header-only mining?

Header-only mining occurs over the Standard Channel and is the simplest mode of the Mining Protocol, making it the least bandwidth and CPU load intensive.
These efficiency gains do come at a cost of miner flexibility, however.

Header-only mining requires the Pool Service to construct the Block Template (meaning the Pool Service selects the transaction set) and minimizes the search space by only allowing for variance of the nonce, version, and time fields by the miner.
RRQ: can they still manipulate the time field?
Before `nTime` rolling, the search space of a Mining Device performing header-only mining is

```
2 ^ (nonce_bits + version_rolling_bits)
2 ^ (32 + 16)
280 x 10^12 bits.
```

In header-only mining, the `extranonce` field in the coinbase transaction is not used, putting the burden of the Merkle root construction fully on the pool.
This is in contrast with the Stratum V1 protocol, which typically sends two parts of the coinbase transaction (separated because of the `extranonce` field that the miner would use as extra search space) and the Merkle branches required for the miner to construct the Merkle root field.
This is also in contrast to Stratum V2's Extended Channel mode which allows for more search space flexibility and/or miner transaction selection.
This simplified means of mining requires the Pool Service to perform most of the block template construction, reducing the miner's CPU load. Additionally, as the Merkle root construction is performed by the Pool Service, less data is transferred to the miner, reducing bandwidth consumption and decreasing a miner's stale job rate.

## Job Selection

## Job Distribution Latency

## Empty Block Mining Elimination

## Multiplexing

## Native Version Rolling

## Vendor Specific Extensions

## Swarm Algorithm
