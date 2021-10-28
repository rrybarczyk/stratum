# Stratum

[Stratum Project page](https://stratum-mining.github.io/)
[Stratum V2 Protocol Specification](https://docs.google.com/document/d/1FadCWj-57dvhxsnFM_7X806qyvhR0u3i85607bGHxvg)
[Stratum V2 High Level Summary](https://docs.google.com/document/d/1efN1JnAGiTw96LtfP70BngtP49SLtbMahPYSROfiMqA)


## Contents
The stratum monorepo is divided into the following modules:

* `protocols`: The implementation of Stratum V1 and all Stratum V2 subprotocols, and their accompanying utilities
* `roles`: The implementation of the Stratum V2 roles
* `utils`: A collection of generic utilities (e.g. async adaptors)
* `examples`: A collection of examples of how to use the crates inside this repo, each example is a binary repo and can be tested with `cargo run`
  * `interop-cpp`
  * `interop-cpp-no-cargo`
  * `ping-pong-with-noise`
  * `ping-pong-without-noise`
  * `sv1-client-and-server`
  * `sv2-proxy`
* `test`: General tests (e.g. if crate x can be built on guix, if crate x can be called by C)


### Protocols
The `protocols` module is composed of the following crates:

* [**serde_sv2**](/protocols/v2/serde-sv2/src/lib.rs): serde serializer and deserializer for the custom binary format, also exports the
    Sv2 primitive data types
* [**noise_sv2**](/protocols/v2/noise-sv2/src/lib.rs): used to encrypt and decrypt Sv2 messages
* [**codec_sv2**](/protocols/v2/codec-sv2/src/lib.rs]): encode and decode Sv2 frames
* [**framing_sv2**](/protocols/v2/framing-sv2/src/lib.rs): Sv2 frame definition and helpers
* [**common_messages**](/protocols/v2/subprotocols/common-messages/src/lib.rs): connection messages used by every (sub)protocol
* [**mining-protocol**](/protocols/v2/subprotocols/mining/src/lib.rs): the miner protocol as defined by the Sv2 specs
* [**job-negotiation-protocol**](/protocols/v2/subprotocols/job-negotiation/src/lib.rs): the job negotiation protocol as defined by the Sv2 specs
* [**template-distribution-protocol**](/protocols/v2/subprotocols/template-distribution/src/lib.rs): the template distribution protocol as defined by the Sv2 specs

### Sv1

Exports `IsServer` and `IsClient` traits. A client that implement `IsClient` will be a correct Sv1
client and a server implementing `IsServer` will be a correct Sv1 server. This library do not
assume async framework and do not IO, the implementor must decide how to do IO and how to manage
multiple connection. An example of implementation is in `protocols/v1/examples/client-and-server`,
to run the example: `cargo run v1`. To run the test: `cargo test v1`

Stratum v1 do not use any kind of encryption. Stratum v1 data format is json so `serde-json` is
imported. Stratum v1 is defined over json-rpc protocol so inside the v1 crate there is a very simple
json-rpc module.

## Examples

### Ping pong with noise
To run [ping pong with noise](/examples/ping-pong-with-noise/README.md)

1. clone this repo: `git clone git@github.com:stratum-mining/stratum.git`
2. go in the right directory: `cd ./stratum/examples/ping-pong-with-noise`
3. run with cargo: `cargo run`

### Ping pong without noise
To run [ping pong without noise](/examples/ping-pong-without-noise/README.md)

1. clone this repo: `git clone git@github.com:stratum-mining/stratum.git`
2. go in the right directory: `cd ./stratum/examples/ping-pong-without-noise`
3. run with cargo: `cargo run`

### Sv1 client and server
To run [Sv1 client and server](/examples/sv1-client-and-server/src/bin.rs)

1. clone this repo: `git clone git@github.com:stratum-mining/stratum.git`
2. go in the right directory: `cd ./stratum/examples/sv1-client-and-server`
3. run with cargo: `cargo run`

## Contrib

### Merging policy

Usually an optimistic merging policy is adopted but in particular cases the contribution must be
reviewed:
* Code is not easy to understand, in this case the reviewer should indicate a simpler implementation.
* It add a dependency, in this case a discussion about the new dependency is needed.
* It modify the build or deploy process.

For everything else including performance an safety issues just accept the PR then amend the
problematic code and do another PR tagging the author of the amended PR.
