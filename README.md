# Pure Rust ROS 1 Serial Implementation

This is a pure-Rust partial re-implementation of [rosserial], more specifically of [rosserial_python].

This re-implementation is *incomplete* and was done purely as a PoC and to understand the protocol.

See the implementation for [`RosSerialMsgCodec`] for details on the protocol. All messages start with `\xff`, normal ROS
messages are serialised as usual and then sent with some additional length & checksum data. Besides normal messages it
has two special sequences to request the list of topics and a final "tx stop" message (not implemented here).

Note that ROS 1 is EOL by May 2025, thus this is largely relevant when migrating to ROS 2 (for which no rosserial
implementation exists at this time).

There is zero support for this crate - feel free to use it as a basis for your own project and feel free to claim the
name on crates.io (this crate hasn't been published). Please do let me know in that case, though :)

## License
Licensed under either of

* Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or <http://www.apache.org/licenses/LICENSE-2.0>)
* MIT license ([LICENSE-MIT](LICENSE-MIT) or <http://opensource.org/licenses/MIT>)

at your option.

## Contribution
Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.

[rosserial]: https://wiki.ros.org/rosserial
[rosserial_python]: https://github.com/ros-drivers/rosserial/tree/noetic-devel/rosserial_python
[`RosSerialMsgCodec`]: src/codec.rs
