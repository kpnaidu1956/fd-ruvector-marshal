//! # Kuiper Belt Self-Learning DBSCAN Analysis with AgenticDB
//!
//! This example demonstrates density-based spatial clustering of Trans-Neptunian Objects
//! using ruvector's AgenticDB for self-learning pattern discovery.
//!
//! ## Features:
//! - DBSCAN clustering optimized for orbital parameter space
//! - Self-learning via reflexion episodes and skills library
//! - Topological Data Analysis (TDA) for cluster quality
//! - Novel discovery detection for potential new dynamical families
//!
//! Run with:
//! ```bash
//! cargo run -p ruvector-core --example kuiper_belt_agenticdb --features storage
//! ```

use ruvector_core::{AgenticDB, Result, VectorEntry};
use ruvector_core::types::DbOptions;
use ruvector_core::advanced::tda::TopologicalAnalyzer;
use std::collections::{HashMap, HashSet};
use std::f32::consts::PI;

/// Kuiper Belt Object orbital elements
#[derive(Debug, Clone)]
pub struct KBO {
    pub name: String,
    pub a: f32,   // Semi-major axis (AU)
    pub e: f32,   // Eccentricity
    pub i: f32,   // Inclination (degrees)
    pub q: f32,   // Perihelion (AU)
    pub ad: f32,  // Aphelion (AU)
}

impl KBO {
    /// Normalized feature vector for clustering
    pub fn to_features(&self) -> Vec<f32> {
        vec![
            self.a / 100.0,   // Semi-major axis normalized
            self.e,           // Eccentricity 0-1
            self.i / 90.0,    // Inclination normalized
            self.q / 100.0,   // Perihelion normalized
        ]
    }

    /// Tisserand parameter w.r.t. Neptune
    pub fn tisserand(&self) -> f32 {
        let a_n = 30.07;
        let term1 = a_n / self.a;
        let term2 = 2.0 * ((self.a / a_n) * (1.0 - self.e.powi(2))).sqrt()
                       * (self.i * PI / 180.0).cos();
        term1 + term2
    }
}

/// Cluster significance classification
#[derive(Debug, Clone, PartialEq)]
pub enum ClusterType {
    Plutino,
    Twotino,
    ColdClassical,
    HotClassical,
    ScatteredDisk,
    Detached,
    NovelDiscovery(String),
}

impl std::fmt::Display for ClusterType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ClusterType::Plutino => write!(f, "🔴 3:2 Neptune Resonance (Plutinos)"),
            ClusterType::Twotino => write!(f, "🟠 2:1 Neptune Resonance (Twotinos)"),
            ClusterType::ColdClassical => write!(f, "🔵 Cold Classical Belt"),
            ClusterType::HotClassical => write!(f, "🟢 Hot Classical Belt"),
            ClusterType::ScatteredDisk => write!(f, "🟣 Scattered Disk"),
            ClusterType::Detached => write!(f, "🟤 Detached/Extreme"),
            ClusterType::NovelDiscovery(desc) => write!(f, "⭐ NOVEL: {}", desc),
        }
    }
}

/// Cluster result with statistics
#[derive(Debug, Clone)]
pub struct Cluster {
    pub id: usize,
    pub members: Vec<String>,
    pub avg_a: f32,
    pub avg_e: f32,
    pub avg_i: f32,
    pub cluster_type: ClusterType,
    pub density: f32,
}

/// DBSCAN implementation
pub fn dbscan(objects: &[KBO], epsilon: f32, min_points: usize) -> (Vec<Cluster>, Vec<String>) {
    let n = objects.len();
    let features: Vec<Vec<f32>> = objects.iter().map(|o| o.to_features()).collect();

    let mut labels = vec![-1i32; n];
    let mut cluster_id = 0i32;

    for i in 0..n {
        if labels[i] != -1 {
            continue;
        }

        let neighbors = range_query(&features, i, epsilon);
        if neighbors.len() < min_points {
            labels[i] = -2; // Noise
        } else {
            expand_cluster(i, &neighbors, cluster_id, &features, &mut labels, epsilon, min_points);
            cluster_id += 1;
        }
    }

    // Build clusters
    let mut cluster_map: HashMap<i32, Vec<usize>> = HashMap::new();
    let mut noise = Vec::new();

    for (idx, &label) in labels.iter().enumerate() {
        if label >= 0 {
            cluster_map.entry(label).or_default().push(idx);
        } else if label == -2 {
            noise.push(objects[idx].name.clone());
        }
    }

    let clusters: Vec<Cluster> = cluster_map
        .into_iter()
        .map(|(id, indices)| {
            let members: Vec<String> = indices.iter().map(|&i| objects[i].name.clone()).collect();
            let avg_a = indices.iter().map(|&i| objects[i].a).sum::<f32>() / indices.len() as f32;
            let avg_e = indices.iter().map(|&i| objects[i].e).sum::<f32>() / indices.len() as f32;
            let avg_i = indices.iter().map(|&i| objects[i].i).sum::<f32>() / indices.len() as f32;

            let member_features: Vec<&Vec<f32>> = indices.iter().map(|&i| &features[i]).collect();
            let density = compute_density(&member_features);
            let cluster_type = classify_cluster(avg_a, avg_e, avg_i, members.len());

            Cluster {
                id: id as usize,
                members,
                avg_a,
                avg_e,
                avg_i,
                cluster_type,
                density,
            }
        })
        .collect();

    (clusters, noise)
}

fn range_query(features: &[Vec<f32>], point_idx: usize, epsilon: f32) -> Vec<usize> {
    let point = &features[point_idx];
    features
        .iter()
        .enumerate()
        .filter(|(_, f)| weighted_distance(point, f) <= epsilon)
        .map(|(i, _)| i)
        .collect()
}

fn expand_cluster(
    point_idx: usize,
    neighbors: &[usize],
    cluster_id: i32,
    features: &[Vec<f32>],
    labels: &mut [i32],
    epsilon: f32,
    min_points: usize,
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
            labels[q_idx] = cluster_id;
        }

        if labels[q_idx] != -1 {
            continue;
        }

        labels[q_idx] = cluster_id;

        let q_neighbors = range_query(features, q_idx, epsilon);
        if q_neighbors.len() >= min_points {
            seed_set.extend(q_neighbors);
        }
    }
}

fn weighted_distance(a: &[f32], b: &[f32]) -> f32 {
    let weights = [2.0, 1.5, 1.5, 1.0];
    a.iter()
        .zip(b.iter())
        .zip(weights.iter())
        .map(|((x, y), w)| w * (x - y).powi(2))
        .sum::<f32>()
        .sqrt()
}

fn compute_density(features: &[&Vec<f32>]) -> f32 {
    if features.len() < 2 {
        return 0.0;
    }

    let mut total_dist = 0.0;
    let mut count = 0;

    for i in 0..features.len() {
        for j in (i + 1)..features.len() {
            total_dist += weighted_distance(features[i], features[j]);
            count += 1;
        }
    }

    if count > 0 {
        1.0 / (1.0 + total_dist / count as f32)
    } else {
        0.0
    }
}

fn classify_cluster(avg_a: f32, avg_e: f32, avg_i: f32, count: usize) -> ClusterType {
    // 3:2 Neptune resonance (Plutinos) at ~39.4 AU
    if (avg_a - 39.4).abs() < 2.0 && avg_e < 0.4 {
        return ClusterType::Plutino;
    }

    // 2:1 Neptune resonance (Twotinos) at ~47.8 AU
    if (avg_a - 47.8).abs() < 2.0 {
        return ClusterType::Twotino;
    }

    // Cold Classical Belt: 42-48 AU, low e, low i
    if avg_a >= 42.0 && avg_a <= 48.0 && avg_e < 0.15 && avg_i < 5.0 {
        return ClusterType::ColdClassical;
    }

    // Hot Classical Belt: 42-48 AU, moderate e/i
    if avg_a >= 42.0 && avg_a <= 48.0 && avg_e < 0.25 {
        return ClusterType::HotClassical;
    }

    // Scattered Disk: high a, high e
    if avg_a > 50.0 && avg_e > 0.3 {
        return ClusterType::ScatteredDisk;
    }

    // Detached objects
    if avg_a > 80.0 {
        return ClusterType::Detached;
    }

    // Novel discoveries
    if avg_i > 40.0 && count >= 2 {
        return ClusterType::NovelDiscovery("High-inclination grouping".to_string());
    }
    if avg_e > 0.7 && avg_a < 60.0 && count >= 2 {
        return ClusterType::NovelDiscovery("High-eccentricity inner cluster".to_string());
    }
    if avg_a > 100.0 && avg_e < 0.5 && count >= 2 {
        return ClusterType::NovelDiscovery("Distant low-e cluster".to_string());
    }

    ClusterType::HotClassical // Default
}

/// Pre-loaded KBO data from NASA/JPL SBDB
fn get_kbo_data() -> Vec<KBO> {
    vec![
        // Dwarf Planets
        KBO { name: "134340 Pluto".into(), a: 39.59, e: 0.2518, i: 17.15, q: 29.619, ad: 49.56 },
        KBO { name: "136199 Eris".into(), a: 68.0, e: 0.4370, i: 43.87, q: 38.284, ad: 97.71 },
        KBO { name: "136108 Haumea".into(), a: 43.01, e: 0.1958, i: 28.21, q: 34.586, ad: 51.42 },
        KBO { name: "136472 Makemake".into(), a: 45.51, e: 0.1604, i: 29.03, q: 38.210, ad: 52.81 },
        KBO { name: "225088 Gonggong".into(), a: 66.89, e: 0.5032, i: 30.87, q: 33.235, ad: 100.55 },
        KBO { name: "90377 Sedna".into(), a: 549.5, e: 0.8613, i: 11.93, q: 76.223, ad: 1022.86 },
        KBO { name: "50000 Quaoar".into(), a: 43.15, e: 0.0358, i: 7.99, q: 41.601, ad: 44.69 },
        KBO { name: "90482 Orcus".into(), a: 39.34, e: 0.2217, i: 20.56, q: 30.614, ad: 48.06 },

        // Plutinos
        KBO { name: "15810 Arawn".into(), a: 39.21, e: 0.1141, i: 3.81, q: 34.734, ad: 43.68 },
        KBO { name: "28978 Ixion".into(), a: 39.35, e: 0.2442, i: 19.67, q: 29.740, ad: 48.96 },
        KBO { name: "38628 Huya".into(), a: 39.21, e: 0.2729, i: 15.48, q: 28.513, ad: 49.91 },
        KBO { name: "47171 Lempo".into(), a: 39.72, e: 0.2298, i: 8.40, q: 30.591, ad: 48.85 },
        KBO { name: "208996 Achlys".into(), a: 39.63, e: 0.1748, i: 13.55, q: 32.699, ad: 46.56 },
        KBO { name: "84922 (2003 VS2)".into(), a: 39.71, e: 0.0816, i: 14.76, q: 36.476, ad: 42.95 },
        KBO { name: "455502 (2003 UZ413)".into(), a: 39.43, e: 0.2182, i: 12.04, q: 30.824, ad: 48.03 },
        KBO { name: "15788 (1993 SB)".into(), a: 39.73, e: 0.3267, i: 1.94, q: 26.754, ad: 52.71 },
        KBO { name: "15789 (1993 SC)".into(), a: 39.74, e: 0.1839, i: 5.16, q: 32.433, ad: 47.05 },
        KBO { name: "341520 Mors-Somnus".into(), a: 39.56, e: 0.2702, i: 11.27, q: 28.871, ad: 50.25 },

        // Classical Belt
        KBO { name: "15760 Albion".into(), a: 44.2, e: 0.0725, i: 2.19, q: 40.995, ad: 47.40 },
        KBO { name: "20000 Varuna".into(), a: 43.18, e: 0.0525, i: 17.14, q: 40.909, ad: 45.45 },
        KBO { name: "19521 Chaos".into(), a: 46.11, e: 0.1105, i: 12.02, q: 41.013, ad: 51.20 },
        KBO { name: "79360 Sila-Nunam".into(), a: 44.04, e: 0.0141, i: 2.24, q: 43.415, ad: 44.66 },
        KBO { name: "66652 Borasisi".into(), a: 43.79, e: 0.0849, i: 0.56, q: 40.075, ad: 47.51 },
        KBO { name: "58534 Logos".into(), a: 45.23, e: 0.1227, i: 2.90, q: 39.681, ad: 50.79 },
        KBO { name: "53311 Deucalion".into(), a: 43.89, e: 0.0588, i: 0.37, q: 41.305, ad: 46.47 },
        KBO { name: "120347 Salacia".into(), a: 42.11, e: 0.1034, i: 23.93, q: 37.761, ad: 46.47 },
        KBO { name: "145452 Ritona".into(), a: 41.55, e: 0.0239, i: 19.26, q: 40.561, ad: 42.55 },
        KBO { name: "55565 Aya".into(), a: 47.3, e: 0.1277, i: 24.34, q: 41.259, ad: 53.34 },
        KBO { name: "174567 Varda".into(), a: 45.54, e: 0.1430, i: 21.51, q: 39.026, ad: 52.05 },
        KBO { name: "420356 Praamzius".into(), a: 42.89, e: 0.0044, i: 1.09, q: 42.699, ad: 43.08 },
        KBO { name: "148780 Altjira".into(), a: 44.54, e: 0.0568, i: 5.20, q: 42.009, ad: 47.07 },
        KBO { name: "385446 Manwe".into(), a: 43.84, e: 0.1164, i: 2.67, q: 38.733, ad: 48.94 },
        KBO { name: "55636 (2002 TX300)".into(), a: 43.48, e: 0.1213, i: 25.86, q: 38.204, ad: 48.75 },
        KBO { name: "55637 Uni".into(), a: 42.97, e: 0.1464, i: 19.41, q: 36.683, ad: 49.27 },
        KBO { name: "202421 (2005 UQ513)".into(), a: 43.53, e: 0.1452, i: 25.72, q: 37.207, ad: 49.85 },

        // Scattered Disk
        KBO { name: "15874 (1996 TL66)".into(), a: 84.89, e: 0.5866, i: 23.96, q: 35.094, ad: 134.69 },
        KBO { name: "26181 (1996 GQ21)".into(), a: 92.48, e: 0.5874, i: 13.36, q: 38.152, ad: 146.81 },
        KBO { name: "26375 (1999 DE9)".into(), a: 55.5, e: 0.4201, i: 7.61, q: 32.184, ad: 78.81 },
        KBO { name: "84522 (2002 TC302)".into(), a: 55.84, e: 0.2995, i: 35.01, q: 39.113, ad: 72.56 },
        KBO { name: "145480 (2005 TB190)".into(), a: 75.93, e: 0.3912, i: 26.48, q: 46.227, ad: 105.64 },
        KBO { name: "229762 G!kun||'homdima".into(), a: 74.59, e: 0.4961, i: 23.33, q: 37.585, ad: 111.59 },
        KBO { name: "145451 Rumina".into(), a: 92.27, e: 0.6190, i: 28.70, q: 35.160, ad: 149.39 },
        KBO { name: "305543 (2008 QY40)".into(), a: 62.37, e: 0.4097, i: 25.12, q: 36.816, ad: 87.91 },

        // Extreme/Detached
        KBO { name: "148209 (2000 CR105)".into(), a: 228.7, e: 0.8071, i: 22.71, q: 44.117, ad: 413.29 },
        KBO { name: "82158 (2001 FP185)".into(), a: 213.4, e: 0.8398, i: 30.80, q: 34.190, ad: 392.66 },
        KBO { name: "87269 (2000 OO67)".into(), a: 617.9, e: 0.9663, i: 20.05, q: 20.850, ad: 1215.04 },
        KBO { name: "308933 (2006 SQ372)".into(), a: 839.3, e: 0.9711, i: 19.46, q: 24.226, ad: 1654.33 },
        KBO { name: "445473 (2010 VZ98)".into(), a: 159.8, e: 0.7851, i: 4.51, q: 34.356, ad: 285.32 },
        KBO { name: "303775 (2005 QU182)".into(), a: 112.2, e: 0.6696, i: 14.01, q: 37.059, ad: 187.28 },
        KBO { name: "353222 (2009 YD7)".into(), a: 125.7, e: 0.8936, i: 30.77, q: 13.379, ad: 238.05 },
        KBO { name: "437360 (2013 TV158)".into(), a: 114.1, e: 0.6801, i: 31.14, q: 36.482, ad: 191.62 },

        // High Inclination
        KBO { name: "65407 (2002 RP120)".into(), a: 54.53, e: 0.9542, i: 119.37, q: 2.498, ad: 106.57 },
        KBO { name: "127546 (2002 XU93)".into(), a: 66.9, e: 0.6862, i: 77.95, q: 20.991, ad: 112.80 },
        KBO { name: "336756 (2010 NV1)".into(), a: 305.2, e: 0.9690, i: 140.82, q: 9.457, ad: 600.93 },
        KBO { name: "418993 (2009 MS9)".into(), a: 375.7, e: 0.9706, i: 67.96, q: 11.046, ad: 740.43 },

        // Additional objects
        KBO { name: "42355 Typhon".into(), a: 37.71, e: 0.5367, i: 2.43, q: 17.470, ad: 57.94 },
        KBO { name: "65489 Ceto".into(), a: 100.5, e: 0.8238, i: 22.30, q: 17.709, ad: 183.25 },
        KBO { name: "307261 Mani".into(), a: 41.6, e: 0.1487, i: 17.70, q: 35.410, ad: 47.78 },
        KBO { name: "78799 Xewioso".into(), a: 37.69, e: 0.2435, i: 14.34, q: 28.511, ad: 46.86 },
        KBO { name: "119951 (2002 KX14)".into(), a: 38.62, e: 0.0448, i: 0.41, q: 36.893, ad: 40.35 },
        KBO { name: "90568 Goibniu".into(), a: 41.81, e: 0.0760, i: 22.04, q: 38.631, ad: 44.99 },
        KBO { name: "451657 (2012 WD36)".into(), a: 77.71, e: 0.5172, i: 23.68, q: 37.517, ad: 117.90 },
        KBO { name: "386723 (2009 YE7)".into(), a: 44.59, e: 0.1373, i: 29.07, q: 38.472, ad: 50.71 },
        KBO { name: "308193 (2005 CB79)".into(), a: 43.45, e: 0.1428, i: 28.60, q: 37.243, ad: 49.65 },
        KBO { name: "444030 (2004 NT33)".into(), a: 43.42, e: 0.1485, i: 31.21, q: 36.966, ad: 49.86 },
        KBO { name: "230965 (2004 XA192)".into(), a: 47.5, e: 0.2533, i: 38.08, q: 35.472, ad: 59.53 },
        KBO { name: "315530 (2008 AP129)".into(), a: 41.99, e: 0.1427, i: 27.41, q: 36.003, ad: 47.99 },
    ]
}

fn main() -> Result<()> {
    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║  KUIPER BELT SELF-LEARNING CLUSTERING WITH AGENTICDB         ║");
    println!("║            Powered by RuVector AgenticDB                     ║");
    println!("╚══════════════════════════════════════════════════════════════╝\n");

    // Load data
    let objects = get_kbo_data();
    println!("📥 Loaded {} Trans-Neptunian Objects from NASA/JPL SBDB\n", objects.len());

    // Initialize AgenticDB
    println!("🧠 Initializing AgenticDB for self-learning...\n");
    let mut options = DbOptions::default();
    options.dimensions = 4; // 4D feature space
    options.storage_path = "./kuiper_agenticdb.db".to_string();
    let db = AgenticDB::new(options)?;

    // Store object vectors in AgenticDB
    println!("📦 Indexing {} objects in vector database...", objects.len());
    for obj in &objects {
        db.insert(VectorEntry {
            id: Some(obj.name.clone()),
            vector: obj.to_features(),
            metadata: Some({
                let mut meta = HashMap::new();
                meta.insert("name".to_string(), serde_json::json!(obj.name.clone()));
                meta.insert("a".to_string(), serde_json::json!(obj.a));
                meta.insert("e".to_string(), serde_json::json!(obj.e));
                meta.insert("i".to_string(), serde_json::json!(obj.i));
                meta
            }),
        })?;
    }
    println!("   ✓ Indexed {} vectors\n", objects.len());

    // Parameter optimization with learning
    println!("🔍 Running DBSCAN with self-learning parameter optimization...\n");

    let param_combos = vec![
        (0.06, 2), (0.08, 2), (0.10, 2), (0.10, 3),
        (0.12, 2), (0.12, 3), (0.15, 3), (0.18, 3),
    ];

    let mut best_clusters = Vec::new();
    let mut best_noise = Vec::new();
    let mut best_score = 0.0f32;
    let mut best_params = (0.0f32, 0usize);

    for (eps, min_pts) in &param_combos {
        let (clusters, noise) = dbscan(&objects, *eps, *min_pts);

        // Evaluate clustering
        let clustered = objects.len() - noise.len();
        let ratio = clustered as f32 / objects.len() as f32;

        let known_clusters = clusters.iter()
            .filter(|c| matches!(c.cluster_type,
                ClusterType::Plutino | ClusterType::ColdClassical |
                ClusterType::HotClassical | ClusterType::ScatteredDisk))
            .count();

        let novel_clusters = clusters.iter()
            .filter(|c| matches!(c.cluster_type, ClusterType::NovelDiscovery(_)))
            .count();

        let cluster_count_score = if clusters.len() >= 4 && clusters.len() <= 12 { 0.3 } else { 0.1 };

        let score = ratio * 0.3 + cluster_count_score + known_clusters as f32 * 0.1 + novel_clusters as f32 * 0.15;

        println!("   eps={:.2}, min_pts={}: {} clusters, {} noise, score={:.3}",
                 eps, min_pts, clusters.len(), noise.len(), score);

        if score > best_score {
            best_score = score;
            best_clusters = clusters;
            best_noise = noise;
            best_params = (*eps, *min_pts);
        }
    }

    // Store reflexion episode about the analysis
    let analysis_critique = format!(
        "Found {} clusters with eps={}, min_pts={}. {} known resonances identified, {} potential novel discoveries. Noise ratio: {:.1}%",
        best_clusters.len(),
        best_params.0,
        best_params.1,
        best_clusters.iter().filter(|c| matches!(c.cluster_type, ClusterType::Plutino | ClusterType::Twotino)).count(),
        best_clusters.iter().filter(|c| matches!(c.cluster_type, ClusterType::NovelDiscovery(_))).count(),
        best_noise.len() as f32 / objects.len() as f32 * 100.0
    );

    db.store_episode(
        "Kuiper Belt DBSCAN Clustering Analysis".to_string(),
        vec![
            "Loaded TNO orbital data from NASA/JPL".to_string(),
            "Normalized orbital parameters to 4D feature space".to_string(),
            format!("Grid search over {} parameter combinations", param_combos.len()),
            format!("Selected eps={}, min_pts={}", best_params.0, best_params.1),
        ],
        vec![
            format!("Found {} clusters", best_clusters.len()),
            format!("{} objects as noise", best_noise.len()),
            format!("Clustering ratio: {:.1}%", (objects.len() - best_noise.len()) as f32 / objects.len() as f32 * 100.0),
        ],
        analysis_critique,
    )?;

    // Create skills for each identified cluster pattern
    for cluster in &best_clusters {
        let mut params = HashMap::new();
        params.insert("a_center".to_string(), format!("{:.1}", cluster.avg_a));
        params.insert("e_avg".to_string(), format!("{:.2}", cluster.avg_e));
        params.insert("i_avg".to_string(), format!("{:.1}", cluster.avg_i));
        params.insert("count".to_string(), cluster.members.len().to_string());

        db.create_skill(
            format!("KBO_{}", cluster.cluster_type),
            format!("Identify {} objects at a~{:.1} AU, e~{:.2}, i~{:.1}°",
                    cluster.cluster_type, cluster.avg_a, cluster.avg_e, cluster.avg_i),
            params,
            vec![
                format!("Check semi-major axis near {:.1} AU", cluster.avg_a),
                format!("Verify eccentricity ~{:.2}", cluster.avg_e),
                format!("Confirm inclination ~{:.1}°", cluster.avg_i),
            ],
        )?;
    }

    // TDA Quality Analysis
    println!("\n🔬 Topological Data Analysis...\n");
    let features: Vec<Vec<f32>> = objects.iter().map(|o| o.to_features()).collect();
    let tda = TopologicalAnalyzer::new(5, best_params.0);
    let quality = tda.analyze(&features)?;

    println!("   Quality score:          {:.3}", quality.quality_score);
    println!("   Clustering coefficient: {:.3}", quality.clustering_coefficient);
    println!("   Connected components:   {}", quality.connected_components);
    println!("   Mode collapse score:    {:.3}", quality.mode_collapse_score);

    // Display results
    println!("\n═══════════════════════════════════════════════════════════════");
    println!("                      CLUSTERING RESULTS                       ");
    println!("═══════════════════════════════════════════════════════════════\n");

    println!("📈 Optimal Parameters: eps={:.2}, min_points={}", best_params.0, best_params.1);
    println!("📊 Total: {} clusters, {} noise ({:.1}% clustered)\n",
             best_clusters.len(), best_noise.len(),
             (objects.len() - best_noise.len()) as f32 / objects.len() as f32 * 100.0);

    println!("🔭 Cluster Classifications:");
    println!("───────────────────────────────────────────────────────────────");

    let mut sorted_clusters = best_clusters.clone();
    sorted_clusters.sort_by(|a, b| b.members.len().cmp(&a.members.len()));

    for cluster in &sorted_clusters {
        println!("   Cluster {:2}: {:3} objects | a={:6.1} AU | e={:.2} | i={:5.1}°",
                 cluster.id, cluster.members.len(), cluster.avg_a, cluster.avg_e, cluster.avg_i);
        println!("              {} | density={:.3}", cluster.cluster_type, cluster.density);
        println!();
    }

    // Novel discoveries
    let novel: Vec<&Cluster> = sorted_clusters.iter()
        .filter(|c| matches!(c.cluster_type, ClusterType::NovelDiscovery(_)))
        .collect();

    if !novel.is_empty() {
        println!("═══════════════════════════════════════════════════════════════");
        println!("                    ⭐ NOVEL DISCOVERIES ⭐                     ");
        println!("═══════════════════════════════════════════════════════════════\n");

        for cluster in novel {
            println!("   {} ({} objects)", cluster.cluster_type, cluster.members.len());
            println!("   Orbital parameters: a={:.1} AU, e={:.2}, i={:.1}°",
                     cluster.avg_a, cluster.avg_e, cluster.avg_i);
            println!("   Members:");
            for member in cluster.members.iter().take(10) {
                if let Some(obj) = objects.iter().find(|o| &o.name == member) {
                    println!("     • {} (a={:.1}, e={:.2}, i={:.1}°)",
                             obj.name, obj.a, obj.e, obj.i);
                }
            }
            println!();

            // Add causal edge for novel discovery
            db.add_causal_edge(
                vec!["clustering_analysis".to_string()],
                vec![format!("novel_cluster_{}", cluster.id)],
                0.85,
                format!("Novel {} discovered via DBSCAN at a~{:.1} AU",
                        cluster.cluster_type, cluster.avg_a),
            )?;
        }

        println!("   Recommended Follow-up:");
        println!("   • Numerical orbit integration for dynamical coherence");
        println!("   • Spectroscopic observations for composition");
        println!("   • Search for additional members in survey data");
    }

    // Query learned skills
    println!("\n🧠 Learned Pattern Recognition Skills:");
    println!("───────────────────────────────────────────────────────────────");
    let learned_skills = db.search_skills("Kuiper Belt orbital patterns", 10)?;
    for skill in learned_skills.iter().take(5) {
        println!("   • {} (success rate: {:.0}%)", skill.name, skill.success_rate * 100.0);
    }

    println!("\n═══════════════════════════════════════════════════════════════");
    println!("                      ANALYSIS COMPLETE                        ");
    println!("   Learned patterns stored in AgenticDB for future analysis    ");
    println!("   Data source: NASA/JPL Small-Body Database                   ");
    println!("═══════════════════════════════════════════════════════════════\n");

    Ok(())
}
