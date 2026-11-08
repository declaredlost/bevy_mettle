//! What a world is over a network, which every part reads.

use bevy_ecs::resource::Resource;

/// What this world is over a network: standalone, the server, or a client.
///
/// Every part does what its role says. Set it once, as the app is built;
/// standalone unless it's set.
#[derive(Resource, Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum NetRole {
    /// Playing alone: everything decided here, nothing replicated.
    #[default]
    Standalone,
    /// The server: everything decided here; what clients show (effect and
    /// ability replicas, served values, cues) kept for them to be sent.
    Server,
    /// A client: what the server owns is shown as it serves it, not decided
    /// here; what this client activates is predicted, kept until the server
    /// confirms it or undone when it rejects it. Once the server has answered
    /// an activation, nothing more is predicted under it: what it does from
    /// then on arrives from the server.
    Client,
}
