pub mod types;

pub mod crypto {
    pub mod encrypt;
    pub mod exchange;
    pub mod identity;
    pub mod noise;
    pub mod sign;
}

pub mod store;

pub mod net {
    pub mod actor;
    pub mod discovery;
    pub mod event_sink;
    pub mod handler;
    pub mod peer_manager;
    pub mod service;
    pub mod transport;
    pub mod wire;

    pub use discovery::{DiscoveredPeer, MdnsDiscovery};
    pub use event_sink::NetEventSink;
    pub use peer_manager::{PeerConnectionInfo, PeerManager};
    pub use service::{NetworkCommand, NetworkService};
    pub use transport::{AuthHello, SecureConnection};
    pub use wire::{
        decode_wire_message, encode_wire_message, frame_message, read_frame_length,
        MAX_FRAME_SIZE,
    };
}

pub mod sync {
    pub mod engine;
    pub mod frontier;
    pub mod lamport;

    pub use engine::SyncEngine;
    pub use frontier::{compute_diff_messages, frontier_contains, merge_frontiers};
    pub use lamport::LamportClock;
}
