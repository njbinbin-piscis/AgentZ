//! Runtime registry for *contributed* (host-supplied) pluggable strategies.
//!
//! The built-in resolvers ([`resolve_loop_strategy`](crate::agent::loop_strategy::resolve_loop_strategy)
//! and [`resolve_compaction_strategy`](crate::agent::loop_::resolve_compaction_strategy))
//! match a fixed set of names. To let a host (e.g. the workbench design
//! assistant) register *new* named strategies at run time without editing those
//! `match` arms, both resolvers fall through to this process-wide registry when
//! their built-in match misses.
//!
//! Entries are factory closures so each `resolve_*` call yields a fresh trait
//! object, matching the built-in behaviour. Registration is idempotent
//! (last-writer-wins) and thread-safe.
//!
//! This is the Rust side of the unified component registry; the Python catalogue
//! (`backend/core/registry.py`) lists these names so they become selectable in
//! configs and the UI once their backing strategy has been built into the
//! binary and registered here (typically from `main`/startup).

use std::collections::HashMap;
use std::sync::{Arc, OnceLock, RwLock};

use crate::agent::compaction_strategy::CompactionStrategy;
use crate::agent::loop_strategy::LoopStrategy;

type LoopFactory = Arc<dyn Fn() -> Arc<dyn LoopStrategy> + Send + Sync>;
type CompactionFactory = Arc<dyn Fn() -> Arc<dyn CompactionStrategy> + Send + Sync>;

#[derive(Default)]
struct ContribRegistry {
    loops: RwLock<HashMap<String, LoopFactory>>,
    compactions: RwLock<HashMap<String, CompactionFactory>>,
}

fn registry() -> &'static ContribRegistry {
    static REGISTRY: OnceLock<ContribRegistry> = OnceLock::new();
    REGISTRY.get_or_init(ContribRegistry::default)
}

/// Register a contributed loop strategy under `name`.
///
/// `factory` is invoked on every successful resolution, so it should be cheap
/// and produce an independent instance.
pub fn register_loop_strategy<F>(name: impl Into<String>, factory: F)
where
    F: Fn() -> Arc<dyn LoopStrategy> + Send + Sync + 'static,
{
    registry()
        .loops
        .write()
        .expect("contrib loop registry poisoned")
        .insert(name.into(), Arc::new(factory));
}

/// Register a contributed compaction strategy under `name`.
pub fn register_compaction_strategy<F>(name: impl Into<String>, factory: F)
where
    F: Fn() -> Arc<dyn CompactionStrategy> + Send + Sync + 'static,
{
    registry()
        .compactions
        .write()
        .expect("contrib compaction registry poisoned")
        .insert(name.into(), Arc::new(factory));
}

/// Resolve a contributed loop strategy by name, if one is registered.
pub fn resolve_loop_strategy(name: &str) -> Option<Arc<dyn LoopStrategy>> {
    let factory = registry()
        .loops
        .read()
        .expect("contrib loop registry poisoned")
        .get(name)
        .cloned();
    factory.map(|f| f())
}

/// Resolve a contributed compaction strategy by name, if one is registered.
pub fn resolve_compaction_strategy(name: &str) -> Option<Arc<dyn CompactionStrategy>> {
    let factory = registry()
        .compactions
        .read()
        .expect("contrib compaction registry poisoned")
        .get(name)
        .cloned();
    factory.map(|f| f())
}

/// Names of all registered contributed loop strategies (sorted).
pub fn loop_strategy_names() -> Vec<String> {
    let mut names: Vec<String> = registry()
        .loops
        .read()
        .expect("contrib loop registry poisoned")
        .keys()
        .cloned()
        .collect();
    names.sort();
    names
}

/// Names of all registered contributed compaction strategies (sorted).
pub fn compaction_strategy_names() -> Vec<String> {
    let mut names: Vec<String> = registry()
        .compactions
        .read()
        .expect("contrib compaction registry poisoned")
        .keys()
        .cloned()
        .collect();
    names.sort();
    names
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::loop_strategy::PromptPatternStrategy;

    #[test]
    fn contrib_loop_round_trips() {
        register_loop_strategy("contrib_test_loop", || {
            Arc::new(PromptPatternStrategy::new("contrib_test_loop"))
        });
        let resolved = resolve_loop_strategy("contrib_test_loop");
        assert!(resolved.is_some());
        assert_eq!(resolved.unwrap().name(), "contrib_test_loop");
        assert!(loop_strategy_names().contains(&"contrib_test_loop".to_string()));
        // Unknown names still miss.
        assert!(resolve_loop_strategy("definitely_not_registered").is_none());
    }
}
