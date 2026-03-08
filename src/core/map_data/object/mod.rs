//! Object-side canonical runtime operations.
//!
//! This compartment owns object load, mutation, query, and index-sync helpers.

pub(super) mod index_sync;
pub(super) mod load;
pub(super) mod mutate;
pub(super) mod query;
