pub mod dashboard;
pub mod handlers;
pub mod models;
pub mod router;
pub mod server;
pub mod state;

// Strategy: Insert Mediator for api (Interface)
// Review and adjust before applying.
pub mod investor;
pub mod worker_ws;
pub mod worker_registry;
