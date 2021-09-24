# Overview
Since its inception in late 2012, the Stratum V1 mining protocol has served a vital role in the Bitcoin mining ecosystem. However the mining landscape as changed extensively in the last decade and a major overhaul to the core mining protocol is well overdue. 
Stratum V2 is positioned as the next iteration of Stratum V1, providing major upgrades from an efficiency, security, and decentralization standpoint.

The purpose of this PR is to provide an understanding of the Stratum V2 protocol with the goal of getting the Template Provider logic required to support Stratum V2, included in Bitcoin Core.

## Design Goals
This section defines the high-level design goals of the Stratum V2 protocol.
All design goals are centered around increasing security, decentralization, and efficiency.

1. Preserve the working aspects of Stratum V1 such that Stratum V2 is logically similar wherever possible. This eases the community transition between the two protocols and makes development simpler.
1. Develop a protocol with a precise definition. There is room for interpretation in the Stratum V1 protocol which resulted in slightly different implementations between pools. Stratum V2 is precisely defined such that implementations will remain consistent and compatible.
1. Authenticate the connection between the downstream miner and the upstream Pool Service. Stratum V1 lacks an encrypted connection between the pool and the miner, leaving the miner vulnerable to a variety of Man in the Middle (MitM) attacks. The most common MitM attack being hashrate hijacking, where an attacker intercepts the mining packets with their credentials, routing the hashrate away from the miner and to the attacker. These attack are very real and are difficult for miners to identify, therefore this is imperative to address. Stratum V2 uses an encrypted connection between the pool and the miner, eliminating these threats. The connection model of Stratum V2 is implemented in such a way that the authentication occurs only once, reducing unnecessary packet transmission realized when using the Stratum V1 protocol.
1. All miners to optionally choose the transaction set to mine on. In Stratum V1, transaction selection is sole responsibility of the Pool Service, something that may lead to transaction censorship and centralization around the Pool Service operators.

## Important Terminology
This section briefly defines important terms that are used throughout this summary.

- Unless otherwise stated, "server-side" refers to the Pool Service and "client-side" refers to a miner controlled downstream role (all roles are defined in the [Role Definitions subsection](https://github.com/rrybarczyk/stratum/blob/bitcoin-core-pr-doc/docs/sv2-explaination.md#role-definitions-wip) below). There are some scenarios in which the miner may not control all roles implemented downstream of the Pool Service, like if another entity was operating the Template Provider (Bitcoin node), but these are less common.

- "Miner" means the organization or individual (mining farm or mining operator) that owns the Mining Device(s) and makes the decisions as to how to configure their Stratum V2 implementation.

- "Proxy" is used as an umbrella term to encompass the roles that sit between the most downstream Mining Device(s) and the most upstream Pool Service. Depending on the configuration chosen by the miner, the term "Proxy" can take on one of two forms:
  1. The Proxy encompasses the Mining Proxy (the implementation of the Mining Protocol) only. This is true when the miner delegates transaction selection to the Pool Service. In this case, the term "Proxy" is synonymous with "Mining Proxy".
  2. The Proxy encompasses both the Mining Proxy and the Job Negotiator (the implementation of the Job Negotiation Protocol). This is true when the miner elects to perform their own transaction selection. In this case the term "Proxy" is synonymous with "Mining Proxy and the Job Negotiator".

  It important to pay close attention to the context in which "Proxy" is being used in order to discern between scenarios in which the Job Negotiator is either included or excluded.

<!-- - Block Template RR TODO -->

<!-- - Difficulty aggregation RR TODO -->

# Protocols and Roles
## Protocol Definitions
The Stratum V2 protocol is split into four subprotocols, allowing for a variety of flexible use cases to suit the needs of different miners. These four subprotocols are defined as follows:

1. The **Mining Protocol** defines the means of communication between the Mining Device(s) and the Pool Service. In its simplest form, the Mining Protocol defines the core mining messages passed between Mining Device(s) (with Stratum V2 compatible firmware) and the Pool Service. These messages perform the familiar tasks associated with mining: opening the connection, accepting new jobs, and submitting work shares. Depending on the miner-chosen configuration however, the Mining Protocol is also used for optional features that allow a miner to make the most out of the Stratum V2 protocol benefits. These features include Mining Device connection aggregation (to reduce bandwidth consumption), translation between Stratum V1 compatible Mining Devices and Stratum V2 compatible Pool Services (so miners can still employ Mining Device(s) with Stratum V1 compatible firmware with Stratum V2 compatible Pool Services), and communication with the Job Negotiator (in the case where the miner elects to perform their own transaction selection). While the Pool Service must be configured to support all of these optional features, whether the miner implements them is optional. Possible configurations are discussed throughout the summary.
The server-side Mining Protocol MUST be implemented by the Pool Service, the Mining Proxy (defined below) role (if used) MUST implement the client-side Mining Protocol that communicates with the server-side Pool Service AND the server-side Mining Protocol that communicates with the Mining Device. Finally, the Mining Device MUST implement the client-side Mining Proxy role.
2. The **Job Negotiation Protocol** is used when the miner elects to perform their own transaction selection. In this case, the Pool Service must agree to the miner selected transaction set, requiring some back and forth communication between the Pool Service and the Proxy. In this way, the process of agreeing on a transaction set is a kind of negotiation, hence the name Job Negotiation Protocol.
The Job Negotiation Protocol MUST be implemented on the server-side by the Pool Service, and MAY be implemented on the client-side by the Job Negotiation role (defined below).
3. The **Template Distribution Protocol** is used only when a miner elects to perform their own transaction set and defines the means of communication between the Job Negotiation Protocol and the Block Template Provider (a Bitcoin node, e.g. `bitcoind`) to extract information about the next candidate block.
The Template Distribution Protocol MUST be implemented by both the Job Negotiator and the Template Distributor (defined below) roles.
4. The **Job Distribution Protocol** is used only when a miner elects to perform their own transaction set and defines the means of communication between the Mining Proxy and the Job Negotiator. This protocol compliments the Job Negotiation Protocol and is responsible for pass the newly negotiated block template to the Mining Proxy which then forwards the work to the interested Mining Device(s).
The Job Distribution Protocol MUST be implemented by the Mining Proxy and the Job Negotiator roles.

## Role Definitions [WIP]
There are five components (types of software or hardware), referred to as roles, that implement these protocols. The five roles are defined as follows:
1. The **Mining Device** is most downstream role and is the physical hardware performing the hash (typically a Bitcoin ASIC).
2. The **Pool Service** is the most upstream role and is responsible for producing jobs (for those not negotiating jobs via the Job Negotiation Protocol), validating shares, and ensuring blocks found by clients are propagated through the network (though clients which have full block templates MUST also propagate blocks into the Bitcoin P2P network). In order for the downstream nodes to use Stratum V2, the Pool Service MUST support Stratum V2. 
3. The **Mining Proxy** is an implementation of the Mining Protocol, which is the intermediary between the downstream Mining Device(s) and the upstream Pool Service. 
4. The **Job Negotiator** is the implementation of the Job Negotiation Protocol and sits "level" with the Mining Proxy in between the downstream Mining Device(s) and the upstream Pool Service. The server-side Job Negotiation Protocol must be implemented by the Pool Service, but the client-side Job Negotiator is optionally used fora miner selected transaction set. The client-side Job Negotiator 
5. The **Template Provider** sits downstream of the Job Negotiator and is a Bitcoin node with Stratum V2 compatible RPC commands to allow for miner transaction selection.
<!-- RR TODO: better template provider definition? -->

## Header Only Mining
<!-- RR TODO: Prob reword this section -->
Header-only mining is the most bandwidth efficient method and easiest on the CPU load.
This is achieved by restricting the miner-controlled search space.
Header-only mining requires the Pool Service to construct the Block Template (meaning the Pool Service selects the transaction set) and minimizes the search space by only allowing for variance of the nonce, version, and time fields by the miner.

Before `nTime` rolling, the search space of a Mining Device performing header-only mining is
```
2 ^ (nonce_bits + version_rolling_bits)
2 ^ (32 + 16)
280 x 10^12 bits
```
In header-only mining, the `extranonce` field in the coinbase transaction is not used, putting the burden of the Merkle root construction fully on the pool.
This is in contrast with the Stratum V1 protocol, which typically sends two parts of the coinbase transaction (separated because of the `extranonce` field that the miner would use as extra search space) and the Merkle branches required for the miner to construct the Merkle root field.
This simplified means of mining requires the Pool Service to perform most of the block template construction, reducing the miner's CPU load. Additionally, as the Merkle root construction is performed by the Pool Service, less data is transferred to the miner, reducing bandwidth consumption and decreasing a miner's stale job rate.

## Communication Channels
As defined in the above [Protocol Definitions section](https://github.com/rrybarczyk/stratum/blob/bitcoin-core-pr-doc/docs/sv2-explaination.md#protocol-definitions), the Mining Protocol defines the means of communication between the Mining Device(s) and the Pool Service. In order to provide maximum flexibility to the miner, three modes of communication, called *channels*, are defined by the Mining Protocol. These three channels are the **standard channel**, **group channel**, and **extended channel**.

### Standard Channel
The simplest communication mode is the standard channel and is strictly used for header-only mining, making it the most efficient channel in terms of bandwidth consumption and CPU load.
To achieve this, some choice is taken away from the miner.
Specifically, the extranonce feature is unused such that the coinbase transaction is not altered and the 32-byte Merkle root hash is sent from the Pool Service to the Mining Protocol node, rather than the Merkle tree branches, reducing bandwidth consumption and CPU load.

The following scenarios describe how a standard channel is typically used.  Note that in all of the following standard channel scenarios, the Mining Device firmware MUST be Stratum V2 compatible.

1. Scenario 1: The miner delegates transaction selection to the Pool Service and the Mining Device firmware is Stratum V2 compatible and supports an encrypted connection directly to the Pool Service. In this case, no Proxy is required as the Mining Device firmware supports the required messages to establish a direct connection to the Pool Service and no Job Negotiator is required as it is up to the Pool Service to select the transactions.

This scenario most closely mirrors today's mining infrastructure landscape and may be appealing to miners who are wiling to forgo some of the core benefits of Stratum V2 (like transaction selection) in favor of simpler infrastructure requirements as no Proxy server is required.


  ```
                    standard
                    channel
    Mining Device ------------> Pool Service
  ```
  It is important to note that it is NOT a requirement of the Stratum V2 protocol for Stratum V2 compatible firmware to support a direct, encrypted connection to the Pool Service.

2. Scenario 2: The miner delegates transaction selection to the Pool Service and the Mining Device firmware is Stratum V2 compatible but does NOT support an encrypted connection directly to the Pool Service. In this case, a Proxy is required to establish an encrypted connection between the Mining Device and the Pool Service. Again, no Job Negotiator is required as it is up to the Pool Service to select the transactions.

  ```
                    standard            standard
                    channel             channel
    Mining Device ------------> Proxy* ------------> Pool Service


    *Proxy: Mining Proxy only
  ```

3. Scenario 3: The miner elects to perform their own transaction set selection and the Mining Device firmware is Stratum V2 compatible. Because the miner is choosing their own transaction set, a Proxy that includes the Job Negotiator is required, implying that it does not matter if the Mining Device firmware supports an encrypted connection directly with the Pool Service since a Proxy is required regardless. 

  ```
                    standard            standard
                    channel             channel
    Mining Device ------------> Proxy* ------------> Pool Service


    *Proxy: Mining Proxy and Job Negotiator
  ```

In any of the above scenarios, more than one Mining Device can be (and most commonly are) used.
This leads to a situation where there is a dedicated connection to the Pool Service for each Mining Device, resulting in unnecessary bandwidth consumption.

  ```
                        standard                  standard
                        channel                   channel
    Mining Device 0   ------------>  +-------+  ------------>  +--------------+
    Mining Device 1   ------------>  |       |  ------------>  |              |
          ...                        |       |                 |              |
    Mining Device n/2 ------------>  | Proxy |  ------------>  | Pool Service |
          ...                        |       |                 |              |
    Mining Device n-1 ------------>  |       |  ------------>  |              |
    Mining Device n   ------------>  +-------+  ------------>  +--------------+
  ```

While this may be an acceptable trade off in scenario 1 given the lighter infrastructure requirements when a Proxy is not used, it may be less acceptable in the second and third scenarios where a miner is operating a Proxy server.Fortunately, Stratum V2 provides means to bundle multiple connections into a single connection to the Pool Service via group channels.
