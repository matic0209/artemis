//! Legacy MEV Arbitrage Modules
//!
//! ⚠️ **DEPRECATED - Migration in Progress**
//!
//! This module contains the previous implementation of the MEV arbitrage system.
//! These modules are being gradually migrated to the new trait-based architecture.
//!
//! ## Status
//!
//! - **MEVOrchestrator** (1299 lines): Event-driven orchestration system
//! - **MEVPipeline** (856 lines): Multi-stage processing pipeline
//! - **HybridArbitrageEngine** (2129 lines): Integrated graph + symbolic + Z3 system
//!
//! ## Why Preserved?
//!
//! These modules contain valuable, battle-tested logic including:
//! - Event bus and priority queue systems
//! - Gas strategy management
//! - Z3 constraint solver integration
//! - Multi-layer validation systems
//! - Performance metrics collection
//!
//! ## Migration Plan
//!
//! See [FEATURE_MIGRATION_PLAN.md](../../../../docs/FEATURE_MIGRATION_PLAN.md) for the
//! detailed roadmap of how functionality will be extracted and migrated to the new architecture.
//!
//! ## Compilation
//!
//! These modules are **not compiled by default** to avoid blocking active development.
//! They can be compiled (with expected errors) using:
//!
//! ```bash
//! cargo check --features full,legacy
//! ```
//!
//! Expected: ~408 compilation errors (being resolved incrementally)
//!
//! ## For New Code
//!
//! **DO NOT** import from this module for new features. Instead:
//! - Use `crate::abstractions::*` for trait definitions
//! - Use `crate::unified_arbitrage_manager::UnifiedArbitrageManager` for orchestration
//! - Use `crate::component_factory::ArbitrageComponentFactory` for component creation
//!
//! ## Timeline
//!
//! - **Phase 1** (Complete): Code preserved in legacy/
//! - **Phase 2** (In Progress): Feature extraction
//! - **Phase 3** (Planned): Adapter layer construction
//! - **Phase 4** (Planned): Gradual replacement
//! - **Phase 5** (Future): Legacy code removal (after ~3 months of validation)

pub mod mev_orchestrator;
pub mod mev_pipeline;
pub mod hybrid_arbitrage_engine;

// Re-export main types for compatibility during migration
pub use mev_orchestrator::MEVOrchestrator;
pub use mev_pipeline::MEVPipeline;
pub use hybrid_arbitrage_engine::HybridArbitrageEngine;