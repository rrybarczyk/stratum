# Summary
Since its inception in late 2012, the Stratum V1 mining protocol has served a vital role in the Bitcoin mining ecosystem. However the mining landscape as changed extensively in the last decade and a major overhaul to the core mining protocol is well overdue. 
Stratum V2 is positioned as the next iteration of Stratum V1, providing major upgrades from both an efficiency and decentralization standpoint.

While the Stratum V2 protocol is a major upgrade, the working features of Stratum V1 are maintained wherever possible with the intention of easing a miner's potential growing pains when making the protocol switch.

RR TODO: explain sv1 and then how sv2 makes improvemnts.
Sv2 is designed to be an improvement to SV1 where SV1 is lacking, the same message types are kept (where appropriate), in an attempt to ease the transition between protocols for miners.
Stratum V1 messages are formatted in plain-text JSON.
This is great for human-readability, and has historically made mining more accessible for new miners. 
However these messages packets are very verbose, resulting in high bandwidth consumption.
Reducing bandwidth consumption is more important now more than ever in order to properly support the highly competitive nature of today's mining ecosystem.
Stratum V2 implements a lean binary protocol which greatly reduces bandwidth consumption, decreasing the stale job rate.

Stratum V1 lacks an encrypted connection between the pool and the miner, leaving the miner vulnerable to a variety of Man in the Middle (MitM) attacks.
The most common MitM attack being hashrate hijacking, where an attacker intercepts the mining packets with their credentials, routing the hashrate away from the miner and to the attacker.
These attack are very real and are difficult for miners to identify, therefore this is imperative to address.
Stratum V2 uses an encrypted connection between the pool and the miner, eliminating these threats.
The connection model of Stratum V2 is implemented in such a way that the authentication occurs only once, reducing unnecessary packet transmission realized when using the Stratum V1 protocol.
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

## Terminology
- Mining Device: Hardware performing the hash, typically a Bitcoin ASIC

- Miner: The individual running Mining Device(s).

- Hashrate Consumer (HC): Upstream node to which shares are submitted to, typically a pool

- Block Template: Block header data fields to be mined over. Includes the Merkle root of the transaction set.

- Mining Proxy: An intermediary node that sits between the Mining Devices and the HC that aggregates connections to increase bandwidth efficiency. The Mining Proxy provides some optional functionality including the ability to monitor the health and performance of the Mining Devices. RRQ: What other extra functionality does the Mining Proxy provide? How is the health of a miner evaluated? Just whether it is on, low, off, or disabled? How is the health different from the performance?
RRTODO: List all the modes - Header-only mining, what are the other modes called?

- Job Negotiator (JN): A node which negotiations with a HC on behalf of one or more Mining Devices to determine which jobs the Mining Devices will work on. This node also communicates with the BTP to select the transaction set, then sends the jobs to the Mining Proxies to be distributed the Mining Devices.

- Block Template Provider (BTP): A Bitcoin node with Stratum V2 compatible RPC commands to allow for transaction selection by the miner. A common example is `bitcoind`.

# Stratum V2 Protocols
## Mining Protocol
### Motivation
This is the direct successor of Stratum V1 and is used for communication between the Mining Devices, Mining Proxies, and Hashrate Consumers.

### Functionality
RR TODO: elaborate on functionality beyond just the channels. Explain that it is the core protocol and provides all the core messaging between the downstream mining farm and the upstream HC.
There are three available modes of communication between the Mining Protocol node and then Hashrate Consumer. These modes are referred to as "channels".

1. **Standard Channels**
This is the simplest mode of communication and is the most efficient in terms of bandwidth consumption and CPU load.
To achieve this, some choice is taken away from the miner.
Specifically, the extranonce feature is unused such that the coinbase transaction is not altered and the 32-byte Merkle root hash is sent from the HC to the Mining Protocol node, rather than the Merkle tree branches, reducing bandwidth consumption and CPU load.
RRQ: is Mining Protocol node the right thing to say here?

2. **Extended Channels**
This mode gives extensive control over the search space used by the Mining Device, allowing the miner to implement advanced used cases such as translation between the Stratum V1 and V2 protocols (used if the HC is Stratum V2 compatible but the Mining Device firmware is Stratum V1 compatible), difficulty aggregation (RRQ: what is diff aggregation? Is it similar to a miner choosing their own pdiff?), custom search space splitting (RRQ: is this encompassing the extranonce? what else does this include?), ect. (RRQ: what else is in "etc"?). The use of these features does come at a cost of higher bandwidth consumption and CPU load compared to the Standard Channel, but is still much more efficient than the Stratum V1 protocol.

3. **Group Channels**
This mode is a collection of Standard Channels that are opened within a particular connection so that they are addressable through a communication channel. RRQ: Need more clarity on this channel type. Is this used for a large farm? Why can't Extended Channels be grouped?


## Job Negotiation (JN) Protocol
### Motivation
The Job Negotiation (JN) Protocol is used when a miner elects to exercise their right to select their own transaction set, a core feature of Stratum V2.
This is in stark contrast to Stratum V1 where only the HC selects the transaction set.
In this way, the pooled mining becomes more akin to solo mining, increasing decentralization.

The ability for a miner to select their own transaction set is vital for the decentralization of Bitcoin.
Currently with Stratum V1, the HC dictates which transactions are included in a block.
This results in a very real threat of transaction censorship by a handful of HCs (e.g. pool operators).
The push for decentralization in this context does come at an increase in overhead cost to the miner because the miner must operate and maintain a Block Template Provider (e.g. a `bitcoind` node).

While some miners may still leave the transaction set selection to the HC, it is vital for the mining ecosystem as a whole to have the ability to select their transaction set in case the treat of transaction censorship by the HC does become a reality.
At a minimum, the Job Negotiation Protocol enables a fail safe for miners to fall back on if this unfortunate situation arises.

## Functionality
The JN Protocol provides the means for a HC to agree upon a Block Template selected by a Miner (RRQ: or should I say TD Protocol?). It is very much a negotiation between the two parties as the Miner sends their selected transaction set to the HC who then either accepts the set or rejects it if it is invalid. 
The result of the negotiation can be reused for all mining connections to the HC. Potentially thousands of Mining Devices can share this connection and this negotiated Block Template, greating reducing computational intensity.

## Template Distribution (TD) Protocol
### Motivation
The Template Distribution (TD) Protocol is used to extract information about the next candidate block from the Block Template Provider (i.e. `bitcoind`), 
replacing the need for `getblocktemplate` (defined in [BIP22](https://github.com/bitcoin/bips/blob/master/bip-0022.mediawiki) and [BIP23](https://github.com/bitcoin/bips/blob/master/bip-0023.mediawiki)).

### Functionality
The Block Template Provider must implement Stratum V2 compatible RPC commands.
RRTODO: What else goes here?

## Job Distribution (JD) Protocol
The Job Distribution (JD) Protocol is used when miners elect to choose their own Block Template. The JD Protocol passes the newly negotiated Block Template to interested nodes, complimenting the JN Protocol.
Here, a node is either the Mining Proxy (in the case where the Mining Device firmware is only Stratum V1 compatible) or the Mining Device itself (in the case where the Mining Device firmware is Stratum V2 compatible).
In the case where miners elect for the HC to choose the Block Template, jobs are distributed directly from the HC to the interested nodes and the JD Protocol is not used.

Additionally, it is possible that the JN role will be part of a larger Mining Protocol proxy that also distributes jobs, making this sub-protocol unnecessary even when miners do choose their own Block Template. RRQ: I have no idea what this means or if it belongs here.

# Stratum V2 Feature Set
## Binary Protocol
Stratum V2 uses a binary protocol as opposed to the plain-text JSON used in Stratum V1.
Using a machine-readable protocol is much more compact than the human-readable alternative, thereby greatly reducing bandwidth consumption for both the upstream HC and the downstream miner, resulting in lower infrastructure costs for all parties and reducing a miner's stale job rate.

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

Header-only mining requires the HC to construct the Block Template (meaning the HC selects the transaction set) and minimizes the search space by only allowing for variance of the nonce, version, and time fields by the miner.
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
This simplified means of mining requires the HC to perform most of the block template construction, reducing the miner's CPU load. Additionally, as the Merkle root construction is performed by the HC, less data is transferred to the miner, reducing bandwidth consumption and decreasing a miner's stale job rate.

## Job Selection

## Job Distribution Latency

## Empty Block Mining Elimination

## Multiplexing

## Native Version Rolling
