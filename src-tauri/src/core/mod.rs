pub mod types;

pub mod crypto {
    pub mod encrypt;
    pub mod exchange;
    pub mod identity;
    pub mod noise;
    pub mod sign;
}

pub mod store {
    pub mod chats;
    pub mod db;
    pub mod messages;
    pub mod outbox;
}

pub mod net {
    pub mod discovery;
    pub mod peer_manager;
    pub mod transport;
    pub mod wire;
}

pub mod sync {
    pub mod engine;
    pub mod frontier;
    pub mod lamport;
}
