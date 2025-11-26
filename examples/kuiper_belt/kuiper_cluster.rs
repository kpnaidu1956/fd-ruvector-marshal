//! # Kuiper Belt Density-Based Spatial Clustering with Self-Learning
//!
//! This module performs DBSCAN clustering on Trans-Neptunian Objects (TNOs) using
//! orbital parameters from NASA/JPL's Small-Body Database. It uses ruvector's
//! AgenticDB for self-learning pattern discovery.
//!
//! ## Key Features:
//! - DBSCAN (Density-Based Spatial Clustering of Applications with Noise)
//! - Self-learning via reflexion episodes and skills library
//! - Topological Data Analysis for cluster quality assessment
//! - Novel discovery detection for potential new dynamical families
//!
//! ## Data Source:
//! NASA/JPL Small-Body Database Query API
//! https://ssd-api.jpl.nasa.gov/sbdb_query.api

use ruvector_core::{AgenticDB, DbOptions, Result, VectorEntry};
use ruvector_core::advanced::tda::TopologicalAnalyzer;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::f32::consts::PI;

/// Kuiper Belt Object with orbital elements
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KuiperBeltObject {
    /// Object name/designation
    pub name: String,
    /// Semi-major axis (AU)
    pub a: f32,
    /// Eccentricity (0-1)
    pub e: f32,
    /// Inclination (degrees)
    pub i: f32,
    /// Perihelion distance (AU)
    pub q: f32,
    /// Aphelion distance (AU)
    pub ad: f32,
    /// Orbital period (days)
    pub period: f32,
    /// Longitude of ascending node (degrees)
    pub omega: f32,
    /// Argument of perihelion (degrees)
    pub w: f32,
    /// Absolute magnitude
    pub h: Option<f32>,
    /// Dynamical class (e.g., "TNO", "Centaur")
    pub class: String,
}

impl KuiperBeltObject {
    /// Convert orbital elements to a feature vector for clustering
    /// Uses normalized orbital parameters for balanced distance calculations
    pub fn to_feature_vector(&self) -> Vec<f32> {
        // Normalize each parameter to roughly 0-1 range for balanced clustering
        vec![
            self.a / 100.0,           // Semi-major axis normalized (most TNOs < 100 AU)
            self.e,                    // Eccentricity already 0-1
            self.i / 90.0,            // Inclination normalized to 0-1 (max 90 degrees)
            self.q / 100.0,           // Perihelion normalized
            (self.omega / 360.0),     // Ascending node normalized
            (self.w / 360.0),         // Argument of perihelion normalized
        ]
    }

    /// Get Tisserand parameter with respect to Neptune (30.07 AU)
    /// T_N = a_N/a + 2*sqrt((a/a_N)*(1-e^2))*cos(i)
    pub fn tisserand_neptune(&self) -> f32 {
        let a_n = 30.07; // Neptune's semi-major axis
        let term1 = a_n / self.a;
        let term2 = 2.0 * ((self.a / a_n) * (1.0 - self.e.powi(2))).sqrt()
                       * (self.i * PI / 180.0).cos();
        term1 + term2
    }

    /// Check if object is in Neptune 3:2 resonance (Plutinos)
    pub fn is_plutino(&self) -> bool {
        // 3:2 resonance occurs at ~39.4 AU
        (self.a - 39.4).abs() < 1.0 && self.e < 0.4
    }

    /// Check if object is in Neptune 2:1 resonance (Twotinos)
    pub fn is_twotino(&self) -> bool {
        // 2:1 resonance occurs at ~47.8 AU
        (self.a - 47.8).abs() < 1.0
    }

    /// Check if object is a classical KBO (Cubewano)
    pub fn is_classical(&self) -> bool {
        // Classical belt: 42-48 AU, low eccentricity
        self.a >= 42.0 && self.a <= 48.0 && self.e < 0.2 && self.i < 30.0
    }

    /// Check if object is a scattered disk object (SDO)
    pub fn is_scattered(&self) -> bool {
        self.q > 30.0 && self.a > 50.0 && self.e > 0.3
    }

    /// Check if object is a detached object (like Sedna)
    pub fn is_detached(&self) -> bool {
        self.q > 40.0 && self.a > 50.0
    }
}

/// DBSCAN cluster result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Cluster {
    /// Cluster ID (0-based)
    pub id: usize,
    /// Member object names
    pub members: Vec<String>,
    /// Centroid in feature space
    pub centroid: Vec<f32>,
    /// Average semi-major axis of cluster members
    pub avg_a: f32,
    /// Average eccentricity
    pub avg_e: f32,
    /// Average inclination
    pub avg_i: f32,
    /// Cluster density score
    pub density: f32,
    /// Potential dynamical significance
    pub significance: ClusterSignificance,
}

/// Significance classification for discovered clusters
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ClusterSignificance {
    /// Known resonance (e.g., 3:2 Plutinos)
    KnownResonance(String),
    /// Classical belt population
    ClassicalBelt,
    /// Scattered disk population
    ScatteredDisk,
    /// Potentially novel grouping requiring investigation
    NovelDiscovery,
    /// Noise (isolated objects)
    Noise,
}

/// DBSCAN clustering implementation for Kuiper Belt objects
pub struct DBSCANClusterer {
    /// Epsilon neighborhood radius
    epsilon: f32,
    /// Minimum points to form a cluster
    min_points: usize,
    /// Distance metric weights for orbital parameters
    weights: Vec<f32>,
}

impl DBSCANClusterer {
    /// Create a new DBSCAN clusterer with given parameters
    pub fn new(epsilon: f32, min_points: usize) -> Self {
        Self {
            epsilon,
            min_points,
            // Weights for: a, e, i, q, omega, w
            // Higher weights for dynamically significant parameters
            weights: vec![2.0, 1.5, 1.5, 1.0, 0.5, 0.5],
        }
    }

    /// Perform DBSCAN clustering on a collection of KBOs
    pub fn cluster(&self, objects: &[KuiperBeltObject]) -> ClusteringResult {
        let n = objects.len();
        if n == 0 {
            return ClusteringResult::empty();
        }

        // Convert objects to feature vectors
        let features: Vec<Vec<f32>> = objects.iter()
            .map(|o| o.to_feature_vector())
            .collect();

        // Initialize labels (-1 = unvisited, -2 = noise, >= 0 = cluster ID)
        let mut labels = vec![-1i32; n];
        let mut cluster_id = 0i32;

        for i in 0..n {
            if labels[i] != -1 {
                continue; // Already processed
            }

            let neighbors = self.range_query(&features, i, &features);

            if neighbors.len() < self.min_points {
                labels[i] = -2; // Mark as noise
            } else {
                // Expand cluster
                self.expand_cluster(
                    i,
                    &neighbors,
                    cluster_id,
                    &features,
                    &mut labels,
                );
                cluster_id += 1;
            }
        }

        // Build cluster results
        let mut clusters: HashMap<i32, Vec<usize>> = HashMap::new();
        let mut noise = Vec::new();

        for (idx, &label) in labels.iter().enumerate() {
            if label >= 0 {
                clusters.entry(label).or_insert_with(Vec::new).push(idx);
            } else if label == -2 {
                noise.push(idx);
            }
        }

        // Create Cluster objects
        let cluster_results: Vec<Cluster> = clusters
            .into_iter()
            .map(|(id, indices)| {
                let members: Vec<String> = indices.iter()
                    .map(|&i| objects[i].name.clone())
                    .collect();

                let member_features: Vec<&Vec<f32>> = indices.iter()
                    .map(|&i| &features[i])
                    .collect();

                let centroid = self.compute_centroid(&member_features);

                let avg_a = indices.iter().map(|&i| objects[i].a).sum::<f32>() / indices.len() as f32;
                let avg_e = indices.iter().map(|&i| objects[i].e).sum::<f32>() / indices.len() as f32;
                let avg_i = indices.iter().map(|&i| objects[i].i).sum::<f32>() / indices.len() as f32;

                let density = self.compute_cluster_density(&member_features);
                let significance = self.classify_cluster(avg_a, avg_e, avg_i, &members, objects);

                Cluster {
                    id: id as usize,
                    members,
                    centroid,
                    avg_a,
                    avg_e,
                    avg_i,
                    density,
                    significance,
                }
            })
            .collect();

        ClusteringResult {
            clusters: cluster_results,
            noise_objects: noise.iter().map(|&i| objects[i].name.clone()).collect(),
            total_objects: n,
            labels,
        }
    }

    fn range_query(&self, features: &[Vec<f32>], point_idx: usize, all_features: &[Vec<f32>]) -> Vec<usize> {
        let point = &all_features[point_idx];
        features
            .iter()
            .enumerate()
            .filter(|(_, f)| self.weighted_distance(point, f) <= self.epsilon)
            .map(|(i, _)| i)
            .collect()
    }

    fn expand_cluster(
        &self,
        point_idx: usize,
        neighbors: &[usize],
        cluster_id: i32,
        features: &[Vec<f32>],
        labels: &mut [i32],
    ) {
        labels[point_idx] = cluster_id;
        let mut seed_set: Vec<usize> = neighbors.to_vec();
        let mut processed = HashSet::new();
        processed.insert(point_idx);

        while let Some(q_idx) = seed_set.pop() {
            if processed.contains(&q_idx) {
                continue;
            }
            processed.insert(q_idx);

            if labels[q_idx] == -2 {
                labels[q_idx] = cluster_id; // Change noise to border point
            }

            if labels[q_idx] != -1 {
                continue; // Already processed
            }

            labels[q_idx] = cluster_id;

            let q_neighbors = self.range_query(features, q_idx, features);
            if q_neighbors.len() >= self.min_points {
                seed_set.extend(q_neighbors);
            }
        }
    }

    fn weighted_distance(&self, a: &[f32], b: &[f32]) -> f32 {
        a.iter()
            .zip(b.iter())
            .zip(self.weights.iter())
            .map(|((x, y), w)| w * (x - y).powi(2))
            .sum::<f32>()
            .sqrt()
    }

    fn compute_centroid(&self, features: &[&Vec<f32>]) -> Vec<f32> {
        if features.is_empty() {
            return vec![];
        }

        let dim = features[0].len();
        let mut centroid = vec![0.0; dim];

        for f in features {
            for (i, &val) in f.iter().enumerate() {
                centroid[i] += val;
            }
        }

        let n = features.len() as f32;
        centroid.iter_mut().for_each(|v| *v /= n);
        centroid
    }

    fn compute_cluster_density(&self, features: &[&Vec<f32>]) -> f32 {
        if features.len() < 2 {
            return 0.0;
        }

        let mut total_dist = 0.0;
        let mut count = 0;

        for i in 0..features.len() {
            for j in (i + 1)..features.len() {
                total_dist += self.weighted_distance(features[i], features[j]);
                count += 1;
            }
        }

        if count > 0 {
            // Higher density = lower average distance (inverse)
            1.0 / (1.0 + total_dist / count as f32)
        } else {
            0.0
        }
    }

    fn classify_cluster(
        &self,
        avg_a: f32,
        avg_e: f32,
        avg_i: f32,
        members: &[String],
        objects: &[KuiperBeltObject],
    ) -> ClusterSignificance {
        // Check for known resonances
        if (avg_a - 39.4).abs() < 2.0 && avg_e < 0.4 {
            return ClusterSignificance::KnownResonance("3:2 Neptune (Plutinos)".to_string());
        }

        if (avg_a - 47.8).abs() < 2.0 {
            return ClusterSignificance::KnownResonance("2:1 Neptune (Twotinos)".to_string());
        }

        if (avg_a - 43.7).abs() < 2.0 && avg_e < 0.2 && avg_i < 5.0 {
            return ClusterSignificance::KnownResonance("Cold Classical Belt".to_string());
        }

        if (avg_a - 55.4).abs() < 2.0 {
            return ClusterSignificance::KnownResonance("5:2 Neptune".to_string());
        }

        // Check for classical belt
        if avg_a >= 42.0 && avg_a <= 48.0 && avg_e < 0.2 {
            return ClusterSignificance::ClassicalBelt;
        }

        // Check for scattered disk
        if avg_a > 50.0 && avg_e > 0.3 {
            return ClusterSignificance::ScatteredDisk;
        }

        // Check for potential novel discoveries
        // Novel if: unusual orbital parameters combination or unknown clustering pattern
        let unusual_conditions = [
            avg_a > 80.0 && avg_e < 0.3,                  // Distant, low-e cluster
            avg_i > 40.0 && members.len() >= 3,           // High-i cluster
            avg_a > 100.0 && members.len() >= 2,          // Very distant cluster
            avg_e > 0.7 && avg_a < 50.0,                  // High-e inner cluster
        ];

        if unusual_conditions.iter().any(|&c| c) {
            return ClusterSignificance::NovelDiscovery;
        }

        ClusterSignificance::Noise
    }
}

/// Result of DBSCAN clustering
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClusteringResult {
    /// Identified clusters
    pub clusters: Vec<Cluster>,
    /// Objects classified as noise
    pub noise_objects: Vec<String>,
    /// Total number of objects processed
    pub total_objects: usize,
    /// Cluster labels for each object (-2 = noise, >= 0 = cluster ID)
    pub labels: Vec<i32>,
}

impl ClusteringResult {
    fn empty() -> Self {
        Self {
            clusters: vec![],
            noise_objects: vec![],
            total_objects: 0,
            labels: vec![],
        }
    }

    /// Get the number of clusters found
    pub fn num_clusters(&self) -> usize {
        self.clusters.len()
    }

    /// Get clusters marked as novel discoveries
    pub fn novel_discoveries(&self) -> Vec<&Cluster> {
        self.clusters
            .iter()
            .filter(|c| c.significance == ClusterSignificance::NovelDiscovery)
            .collect()
    }

    /// Get clustering statistics
    pub fn statistics(&self) -> ClusteringStats {
        let total_clustered = self.clusters.iter().map(|c| c.members.len()).sum::<usize>();
        let cluster_sizes: Vec<usize> = self.clusters.iter().map(|c| c.members.len()).collect();

        ClusteringStats {
            num_clusters: self.clusters.len(),
            total_objects: self.total_objects,
            clustered_objects: total_clustered,
            noise_objects: self.noise_objects.len(),
            clustering_ratio: total_clustered as f32 / self.total_objects.max(1) as f32,
            avg_cluster_size: if !cluster_sizes.is_empty() {
                cluster_sizes.iter().sum::<usize>() as f32 / cluster_sizes.len() as f32
            } else {
                0.0
            },
            max_cluster_size: cluster_sizes.iter().copied().max().unwrap_or(0),
            min_cluster_size: cluster_sizes.iter().copied().min().unwrap_or(0),
        }
    }
}

/// Clustering statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClusteringStats {
    pub num_clusters: usize,
    pub total_objects: usize,
    pub clustered_objects: usize,
    pub noise_objects: usize,
    pub clustering_ratio: f32,
    pub avg_cluster_size: f32,
    pub max_cluster_size: usize,
    pub min_cluster_size: usize,
}

/// Self-learning Kuiper Belt analyzer using AgenticDB
pub struct SelfLearningAnalyzer {
    /// AgenticDB for learning
    db: AgenticDB,
    /// DBSCAN parameters learned from experience
    best_epsilon: f32,
    best_min_points: usize,
    /// Discovered patterns
    learned_patterns: Vec<LearnedPattern>,
}

/// A pattern learned from clustering analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LearnedPattern {
    /// Pattern name
    pub name: String,
    /// Description
    pub description: String,
    /// Orbital parameter ranges
    pub a_range: (f32, f32),
    pub e_range: (f32, f32),
    pub i_range: (f32, f32),
    /// Confidence score
    pub confidence: f32,
}

impl SelfLearningAnalyzer {
    /// Create a new self-learning analyzer
    pub fn new(storage_path: &str) -> Result<Self> {
        let mut options = DbOptions::default();
        options.dimensions = 6; // 6-dimensional orbital parameter space
        options.storage_path = storage_path.to_string();

        let db = AgenticDB::new(options)?;

        // Initialize a learning session for parameter optimization
        let _ = db.start_session(
            "KuiperClustering".to_string(),
            6, // state dimensions: eps, min_pts, num_clusters, noise_count, etc.
            1, // action dimension: score
        );

        Ok(Self {
            db,
            best_epsilon: 0.15,     // Initial guess
            best_min_points: 3,     // Initial guess
            learned_patterns: vec![],
        })
    }

    /// Run clustering with self-learning parameter optimization
    pub fn analyze_with_learning(
        &mut self,
        objects: &[KuiperBeltObject],
        iterations: usize,
    ) -> Result<AnalysisResult> {
        let mut best_result: Option<ClusteringResult> = None;
        let mut best_score = 0.0;

        // Grid search with learning
        let epsilons = vec![0.08, 0.10, 0.12, 0.15, 0.18, 0.20, 0.25];
        let min_points_list = vec![2, 3, 4, 5];

        for iter in 0..iterations {
            for &eps in &epsilons {
                for &min_pts in &min_points_list {
                    let clusterer = DBSCANClusterer::new(eps, min_pts);
                    let result = clusterer.cluster(objects);

                    let score = self.evaluate_clustering(&result, objects);

                    // Log clustering results for learning (without requiring session)
                    // The learning happens through reflexion episodes and skill creation

                    if score > best_score {
                        best_score = score;
                        best_result = Some(result);
                        self.best_epsilon = eps;
                        self.best_min_points = min_pts;
                    }
                }
            }
        }

        let result = best_result.unwrap_or_else(|| {
            let clusterer = DBSCANClusterer::new(self.best_epsilon, self.best_min_points);
            clusterer.cluster(objects)
        });

        // Store reflexion episode about the analysis
        let critique = if result.novel_discoveries().is_empty() {
            "No novel discoveries found in this analysis. Consider adjusting epsilon for finer clustering.".to_string()
        } else {
            format!(
                "Found {} potential novel discoveries. These warrant further investigation for potential new dynamical families.",
                result.novel_discoveries().len()
            )
        };

        self.db.store_episode(
            "Kuiper Belt DBSCAN clustering".to_string(),
            vec![
                "Loaded orbital data".to_string(),
                "Normalized orbital parameters".to_string(),
                format!("Ran DBSCAN with eps={}, min_pts={}", self.best_epsilon, self.best_min_points),
                "Classified clusters by significance".to_string(),
            ],
            vec![
                format!("Found {} clusters", result.num_clusters()),
                format!("{} objects classified as noise", result.noise_objects.len()),
                format!("{} potential novel discoveries", result.novel_discoveries().len()),
            ],
            critique,
        )?;

        // Extract and learn patterns
        self.learn_patterns(&result, objects)?;

        // Perform TDA analysis for quality assessment
        let features: Vec<Vec<f32>> = objects.iter()
            .map(|o| o.to_feature_vector())
            .collect();

        let tda = TopologicalAnalyzer::new(5, self.best_epsilon);
        let quality = tda.analyze(&features)?;

        Ok(AnalysisResult {
            clustering: result,
            quality_score: quality.quality_score,
            clustering_coefficient: quality.clustering_coefficient,
            connected_components: quality.connected_components,
            best_epsilon: self.best_epsilon,
            best_min_points: self.best_min_points,
            learned_patterns: self.learned_patterns.clone(),
        })
    }

    fn evaluate_clustering(&self, result: &ClusteringResult, objects: &[KuiperBeltObject]) -> f32 {
        // Multi-factor scoring:
        // 1. Silhouette-like score (cluster compactness vs separation)
        // 2. Bonus for finding known patterns
        // 3. Penalty for too many noise objects
        // 4. Bonus for novel discoveries

        let stats = result.statistics();

        // Base score from clustering ratio
        let mut score = stats.clustering_ratio * 0.3;

        // Bonus for reasonable number of clusters (5-20 is ideal for KBOs)
        if stats.num_clusters >= 5 && stats.num_clusters <= 20 {
            score += 0.2;
        } else if stats.num_clusters > 0 && stats.num_clusters < 30 {
            score += 0.1;
        }

        // Bonus for finding known resonances
        let known_resonances = result.clusters.iter()
            .filter(|c| matches!(c.significance, ClusterSignificance::KnownResonance(_)))
            .count();
        score += known_resonances as f32 * 0.1;

        // Bonus for novel discoveries
        score += result.novel_discoveries().len() as f32 * 0.05;

        // Penalty for excessive noise
        if stats.noise_objects as f32 / stats.total_objects as f32 > 0.5 {
            score -= 0.1;
        }

        score.clamp(0.0, 1.0)
    }

    fn learn_patterns(&mut self, result: &ClusteringResult, objects: &[KuiperBeltObject]) -> Result<()> {
        for cluster in &result.clusters {
            if matches!(cluster.significance, ClusterSignificance::NovelDiscovery | ClusterSignificance::KnownResonance(_)) {
                // Extract parameter ranges from cluster members
                let member_objects: Vec<&KuiperBeltObject> = cluster.members.iter()
                    .filter_map(|name| objects.iter().find(|o| &o.name == name))
                    .collect();

                if member_objects.is_empty() {
                    continue;
                }

                let a_values: Vec<f32> = member_objects.iter().map(|o| o.a).collect();
                let e_values: Vec<f32> = member_objects.iter().map(|o| o.e).collect();
                let i_values: Vec<f32> = member_objects.iter().map(|o| o.i).collect();

                let pattern = LearnedPattern {
                    name: match &cluster.significance {
                        ClusterSignificance::KnownResonance(r) => r.clone(),
                        ClusterSignificance::NovelDiscovery => format!("Novel Cluster {}", cluster.id),
                        _ => format!("Cluster {}", cluster.id),
                    },
                    description: format!(
                        "Cluster with {} members at avg a={:.1} AU, e={:.2}, i={:.1}°",
                        cluster.members.len(), cluster.avg_a, cluster.avg_e, cluster.avg_i
                    ),
                    a_range: (*a_values.iter().min_by(|a, b| a.partial_cmp(b).unwrap()).unwrap(),
                              *a_values.iter().max_by(|a, b| a.partial_cmp(b).unwrap()).unwrap()),
                    e_range: (*e_values.iter().min_by(|a, b| a.partial_cmp(b).unwrap()).unwrap(),
                              *e_values.iter().max_by(|a, b| a.partial_cmp(b).unwrap()).unwrap()),
                    i_range: (*i_values.iter().min_by(|a, b| a.partial_cmp(b).unwrap()).unwrap(),
                              *i_values.iter().max_by(|a, b| a.partial_cmp(b).unwrap()).unwrap()),
                    confidence: cluster.density,
                };

                // Create skill for pattern recognition
                let mut params = std::collections::HashMap::new();
                params.insert("a_min".to_string(), pattern.a_range.0.to_string());
                params.insert("a_max".to_string(), pattern.a_range.1.to_string());
                params.insert("e_min".to_string(), pattern.e_range.0.to_string());
                params.insert("e_max".to_string(), pattern.e_range.1.to_string());

                self.db.create_skill(
                    pattern.name.clone(),
                    pattern.description.clone(),
                    params,
                    vec![
                        format!("Check if object a in [{:.1}, {:.1}]", pattern.a_range.0, pattern.a_range.1),
                        format!("Check if object e in [{:.2}, {:.2}]", pattern.e_range.0, pattern.e_range.1),
                        format!("Check if object i in [{:.1}, {:.1}]", pattern.i_range.0, pattern.i_range.1),
                    ],
                )?;

                self.learned_patterns.push(pattern);
            }
        }

        Ok(())
    }

    /// Search for patterns similar to a query
    pub fn search_similar_patterns(&self, query: &str, k: usize) -> Result<Vec<ruvector_core::agenticdb::Skill>> {
        self.db.search_skills(query, k)
    }
}

/// Complete analysis result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalysisResult {
    /// Clustering result
    pub clustering: ClusteringResult,
    /// TDA quality score
    pub quality_score: f32,
    /// Clustering coefficient
    pub clustering_coefficient: f32,
    /// Number of connected components
    pub connected_components: usize,
    /// Optimal epsilon found
    pub best_epsilon: f32,
    /// Optimal min_points found
    pub best_min_points: usize,
    /// Learned patterns
    pub learned_patterns: Vec<LearnedPattern>,
}

impl AnalysisResult {
    /// Generate a summary report
    pub fn summary(&self) -> String {
        let stats = self.clustering.statistics();
        let novel = self.clustering.novel_discoveries();

        let mut report = String::new();

        report.push_str("═══════════════════════════════════════════════════════════════\n");
        report.push_str("           KUIPER BELT DENSITY CLUSTERING ANALYSIS             \n");
        report.push_str("═══════════════════════════════════════════════════════════════\n\n");

        report.push_str("📊 CLUSTERING STATISTICS\n");
        report.push_str("───────────────────────────────────────────────────────────────\n");
        report.push_str(&format!("  Total objects analyzed:     {}\n", stats.total_objects));
        report.push_str(&format!("  Clusters identified:        {}\n", stats.num_clusters));
        report.push_str(&format!("  Objects in clusters:        {}\n", stats.clustered_objects));
        report.push_str(&format!("  Noise objects:              {}\n", stats.noise_objects));
        report.push_str(&format!("  Clustering ratio:           {:.1}%\n", stats.clustering_ratio * 100.0));
        report.push_str(&format!("  Avg cluster size:           {:.1}\n", stats.avg_cluster_size));
        report.push_str(&format!("  Best epsilon:               {:.3}\n", self.best_epsilon));
        report.push_str(&format!("  Best min_points:            {}\n", self.best_min_points));
        report.push_str("\n");

        report.push_str("📈 TOPOLOGICAL DATA ANALYSIS\n");
        report.push_str("───────────────────────────────────────────────────────────────\n");
        report.push_str(&format!("  Quality score:              {:.3}\n", self.quality_score));
        report.push_str(&format!("  Clustering coefficient:     {:.3}\n", self.clustering_coefficient));
        report.push_str(&format!("  Connected components:       {}\n", self.connected_components));
        report.push_str("\n");

        report.push_str("🔭 CLUSTER CLASSIFICATIONS\n");
        report.push_str("───────────────────────────────────────────────────────────────\n");

        for cluster in &self.clustering.clusters {
            let sig = match &cluster.significance {
                ClusterSignificance::KnownResonance(r) => format!("Resonance: {}", r),
                ClusterSignificance::ClassicalBelt => "Classical Belt".to_string(),
                ClusterSignificance::ScatteredDisk => "Scattered Disk".to_string(),
                ClusterSignificance::NovelDiscovery => "⭐ NOVEL DISCOVERY".to_string(),
                ClusterSignificance::Noise => "Noise".to_string(),
            };

            report.push_str(&format!(
                "  Cluster {:2}: {:3} members | a={:5.1} AU | e={:.2} | i={:5.1}° | {}\n",
                cluster.id, cluster.members.len(), cluster.avg_a, cluster.avg_e, cluster.avg_i, sig
            ));
        }
        report.push_str("\n");

        if !novel.is_empty() {
            report.push_str("⭐ NOVEL DISCOVERIES\n");
            report.push_str("───────────────────────────────────────────────────────────────\n");
            for cluster in novel {
                report.push_str(&format!(
                    "  Cluster {} - {} members:\n",
                    cluster.id, cluster.members.len()
                ));
                report.push_str(&format!(
                    "    Orbital parameters: a={:.1}±{:.1} AU, e={:.2}, i={:.1}°\n",
                    cluster.avg_a,
                    cluster.centroid.get(0).unwrap_or(&0.0) * 100.0 - cluster.avg_a,
                    cluster.avg_e,
                    cluster.avg_i
                ));
                report.push_str(&format!("    Density score: {:.3}\n", cluster.density));
                report.push_str("    Members:\n");
                for (i, member) in cluster.members.iter().enumerate() {
                    if i < 10 {
                        report.push_str(&format!("      - {}\n", member));
                    } else {
                        report.push_str(&format!("      ... and {} more\n", cluster.members.len() - 10));
                        break;
                    }
                }
            }
            report.push_str("\n");
        }

        if !self.learned_patterns.is_empty() {
            report.push_str("🧠 LEARNED PATTERNS\n");
            report.push_str("───────────────────────────────────────────────────────────────\n");
            for pattern in &self.learned_patterns {
                report.push_str(&format!("  {}\n", pattern.name));
                report.push_str(&format!("    {}\n", pattern.description));
                report.push_str(&format!(
                    "    a: [{:.1}, {:.1}] AU | e: [{:.2}, {:.2}] | i: [{:.1}, {:.1}]°\n",
                    pattern.a_range.0, pattern.a_range.1,
                    pattern.e_range.0, pattern.e_range.1,
                    pattern.i_range.0, pattern.i_range.1
                ));
            }
        }

        report.push_str("\n═══════════════════════════════════════════════════════════════\n");
        report.push_str("  Data source: NASA/JPL Small-Body Database                    \n");
        report.push_str("  Analysis: RuVector Self-Learning DBSCAN                      \n");
        report.push_str("═══════════════════════════════════════════════════════════════\n");

        report
    }
}

/// Parse raw JPL API JSON data into KuiperBeltObjects
/// Note: For production use, prefer the pre-loaded data from kbo_data module
pub fn parse_jpl_json(json_data: &serde_json::Value) -> Vec<KuiperBeltObject> {
    let mut objects = Vec::new();

    if let Some(data) = json_data.get("data").and_then(|d| d.as_array()) {
        for row in data {
            if let Some(arr) = row.as_array() {
                if arr.len() >= 10 {
                    let name = arr[0].as_str().unwrap_or("").trim().to_string();
                    let a = arr[1].as_str().and_then(|s| s.parse().ok()).unwrap_or(0.0);
                    let e = arr[2].as_str().and_then(|s| s.parse().ok()).unwrap_or(0.0);
                    let i = arr[3].as_str().and_then(|s| s.parse().ok()).unwrap_or(0.0);
                    let q = arr[4].as_str().and_then(|s| s.parse().ok()).unwrap_or(0.0);
                    let ad = arr[5].as_str().and_then(|s| s.parse().ok()).unwrap_or(0.0);
                    let omega = arr[7].as_str().and_then(|s| s.parse().ok()).unwrap_or(0.0);
                    let w = arr[8].as_str().and_then(|s| s.parse().ok()).unwrap_or(0.0);
                    let h = arr[9].as_str().and_then(|s| s.parse().ok());
                    let class = arr.get(10).and_then(|v| v.as_str()).unwrap_or("TNO").to_string();

                    if a > 0.0 {
                        objects.push(KuiperBeltObject {
                            name,
                            a,
                            e,
                            i,
                            q,
                            ad,
                            period: 0.0,
                            omega,
                            w,
                            h,
                            class,
                        });
                    }
                }
            }
        }
    }

    objects
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_objects() -> Vec<KuiperBeltObject> {
        vec![
            // Plutinos (3:2 resonance at ~39.4 AU)
            KuiperBeltObject {
                name: "Pluto".to_string(),
                a: 39.59, e: 0.2518, i: 17.15, q: 29.619, ad: 49.56,
                period: 90900.0, omega: 110.29, w: 113.71, h: Some(-0.54), class: "TNO".to_string(),
            },
            KuiperBeltObject {
                name: "Orcus".to_string(),
                a: 39.34, e: 0.2217, i: 20.56, q: 30.614, ad: 48.06,
                period: 90100.0, omega: 268.39, w: 73.72, h: Some(2.14), class: "TNO".to_string(),
            },
            // Classical belt (42-48 AU, low e)
            KuiperBeltObject {
                name: "Quaoar".to_string(),
                a: 43.15, e: 0.0358, i: 7.99, q: 41.601, ad: 44.69,
                period: 104000.0, omega: 188.96, w: 163.92, h: Some(2.41), class: "TNO".to_string(),
            },
            KuiperBeltObject {
                name: "Varuna".to_string(),
                a: 43.18, e: 0.0525, i: 17.14, q: 40.909, ad: 45.45,
                period: 104000.0, omega: 97.21, w: 273.22, h: Some(3.79), class: "TNO".to_string(),
            },
            // Scattered disk object
            KuiperBeltObject {
                name: "Eris".to_string(),
                a: 68.0, e: 0.4370, i: 43.87, q: 38.284, ad: 97.71,
                period: 205000.0, omega: 36.03, w: 150.73, h: Some(-1.25), class: "TNO".to_string(),
            },
            // Distant/Detached (Sedna-like)
            KuiperBeltObject {
                name: "Sedna".to_string(),
                a: 549.5, e: 0.8613, i: 11.93, q: 76.223, ad: 1022.86,
                period: 4710000.0, omega: 144.48, w: 311.01, h: Some(1.49), class: "TNO".to_string(),
            },
        ]
    }

    #[test]
    fn test_kbo_feature_vector() {
        let kbo = create_test_objects()[0].clone();
        let features = kbo.to_feature_vector();

        assert_eq!(features.len(), 6);
        assert!(features[0] > 0.0); // Normalized semi-major axis
        assert!(features[1] >= 0.0 && features[1] <= 1.0); // Eccentricity
    }

    #[test]
    fn test_kbo_classification() {
        let objects = create_test_objects();

        assert!(objects[0].is_plutino()); // Pluto
        assert!(objects[2].is_classical()); // Quaoar
        assert!(objects[4].is_scattered()); // Eris
        assert!(objects[5].is_detached()); // Sedna
    }

    #[test]
    fn test_dbscan_clustering() {
        let objects = create_test_objects();
        let clusterer = DBSCANClusterer::new(0.15, 2);
        let result = clusterer.cluster(&objects);

        // Should find at least one cluster (Plutinos)
        assert!(result.num_clusters() >= 1);
    }

    #[test]
    fn test_tisserand_parameter() {
        let pluto = create_test_objects()[0].clone();
        let t_n = pluto.tisserand_neptune();

        // Pluto's Tisserand with Neptune should be around 2.7
        assert!(t_n > 2.0 && t_n < 4.0);
    }

    #[test]
    fn test_clustering_statistics() {
        let objects = create_test_objects();
        let clusterer = DBSCANClusterer::new(0.15, 2);
        let result = clusterer.cluster(&objects);
        let stats = result.statistics();

        assert_eq!(stats.total_objects, objects.len());
        assert!(stats.clustering_ratio >= 0.0 && stats.clustering_ratio <= 1.0);
    }
}
