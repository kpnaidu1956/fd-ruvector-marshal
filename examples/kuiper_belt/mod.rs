//! # Kuiper Belt Clustering Analysis Module
//!
//! Self-learning density-based spatial clustering for Trans-Neptunian Objects.
//!
//! ## Features
//! - DBSCAN clustering optimized for orbital parameter space
//! - AgenticDB integration for self-learning pattern discovery
//! - Topological Data Analysis for cluster quality assessment
//! - Novel discovery detection for potential new dynamical families

pub mod kuiper_cluster;
pub mod kbo_data;

pub use kuiper_cluster::{
    KuiperBeltObject,
    DBSCANClusterer,
    SelfLearningAnalyzer,
    Cluster,
    ClusterSignificance,
    ClusteringResult,
    AnalysisResult,
    LearnedPattern,
};

pub use kbo_data::{get_kbo_data, get_sample_kbos};
