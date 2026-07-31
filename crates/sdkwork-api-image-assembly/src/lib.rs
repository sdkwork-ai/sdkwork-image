//! API assembly for sdkwork-image.
//! Application bootstrap lives in `bootstrap.rs`; route inventory is in `assembly-manifest.json`.

mod bootstrap;
mod generated;

pub use bootstrap::{
    assemble_api_router, assemble_api_router_from_env, assemble_api_router_with_pool, ApiAssembly,
};

pub fn assembly_route_count() -> usize {
    generated::ROUTE_CRATE_COUNT
}
