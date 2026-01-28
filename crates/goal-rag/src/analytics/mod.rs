//! Interaction Analytics & Workflow Intelligence System
//!
//! This module provides:
//! - Classification of team communications (comments, messages)
//! - Workflow timeline reconstruction
//! - Pattern learning from successful/failed workflows
//! - Efficiency recommendations

pub mod types;
pub mod storage;
pub mod classifier;
pub mod timeline;
pub mod pattern_learner;
pub mod recommender;
pub mod jobs;

pub use types::*;
pub use storage::AnalyticsDb;
pub use classifier::OllamaClassifier;
