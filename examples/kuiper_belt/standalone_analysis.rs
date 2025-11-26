//! # Standalone Kuiper Belt DBSCAN Clustering Analysis
//!
//! This is a standalone analysis script that performs density-based clustering
//! on Trans-Neptunian Objects without requiring the full AgenticDB infrastructure.
//!
//! Run with: cargo run --example kuiper_standalone --features storage

use std::collections::{HashMap, HashSet};
use std::f32::consts::PI;

/// Kuiper Belt Object with orbital elements
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
    /// Convert to normalized feature vector for clustering
    pub fn to_features(&self) -> Vec<f32> {
        vec![
            self.a / 100.0,   // Normalize semi-major axis
            self.e,           // Eccentricity already 0-1
            self.i / 90.0,    // Normalize inclination
            self.q / 100.0,   // Normalize perihelion
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

/// DBSCAN Clustering Result
#[derive(Debug)]
pub struct ClusterResult {
    pub clusters: Vec<Vec<String>>,
    pub noise: Vec<String>,
    pub cluster_stats: Vec<ClusterStats>,
}

#[derive(Debug, Clone)]
pub struct ClusterStats {
    pub id: usize,
    pub count: usize,
    pub avg_a: f32,
    pub avg_e: f32,
    pub avg_i: f32,
    pub classification: String,
}

/// DBSCAN clustering implementation
pub fn dbscan(objects: &[KBO], epsilon: f32, min_points: usize) -> ClusterResult {
    let n = objects.len();
    let features: Vec<Vec<f32>> = objects.iter().map(|o| o.to_features()).collect();

    // Labels: -1 = unvisited, -2 = noise, >= 0 = cluster ID
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

    // Build results
    let mut clusters: HashMap<i32, Vec<String>> = HashMap::new();
    let mut noise = Vec::new();

    for (idx, &label) in labels.iter().enumerate() {
        if label >= 0 {
            clusters.entry(label).or_default().push(objects[idx].name.clone());
        } else if label == -2 {
            noise.push(objects[idx].name.clone());
        }
    }

    // Calculate cluster statistics
    let cluster_stats: Vec<ClusterStats> = clusters
        .iter()
        .map(|(&id, members)| {
            let member_objects: Vec<&KBO> = members.iter()
                .filter_map(|name| objects.iter().find(|o| &o.name == name))
                .collect();

            let avg_a = member_objects.iter().map(|o| o.a).sum::<f32>() / member_objects.len() as f32;
            let avg_e = member_objects.iter().map(|o| o.e).sum::<f32>() / member_objects.len() as f32;
            let avg_i = member_objects.iter().map(|o| o.i).sum::<f32>() / member_objects.len() as f32;

            let classification = classify_cluster(avg_a, avg_e, avg_i);

            ClusterStats {
                id: id as usize,
                count: members.len(),
                avg_a,
                avg_e,
                avg_i,
                classification,
            }
        })
        .collect();

    ClusterResult {
        clusters: clusters.into_values().collect(),
        noise,
        cluster_stats,
    }
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
    // Weights: a=2.0, e=1.5, i=1.5, q=1.0
    let weights = [2.0, 1.5, 1.5, 1.0];
    a.iter()
        .zip(b.iter())
        .zip(weights.iter())
        .map(|((x, y), w)| w * (x - y).powi(2))
        .sum::<f32>()
        .sqrt()
}

fn classify_cluster(avg_a: f32, avg_e: f32, avg_i: f32) -> String {
    if (avg_a - 39.4).abs() < 2.0 && avg_e < 0.4 {
        return "🔴 3:2 Neptune Resonance (Plutinos)".to_string();
    }
    if (avg_a - 47.8).abs() < 2.0 {
        return "🟠 2:1 Neptune Resonance (Twotinos)".to_string();
    }
    if (avg_a - 43.7).abs() < 2.0 && avg_e < 0.15 && avg_i < 5.0 {
        return "🔵 Cold Classical Belt".to_string();
    }
    if avg_a >= 42.0 && avg_a <= 48.0 && avg_e < 0.2 {
        return "🟢 Classical Kuiper Belt".to_string();
    }
    if avg_a > 50.0 && avg_e > 0.3 {
        return "🟣 Scattered Disk".to_string();
    }
    if avg_a > 100.0 || avg_i > 40.0 {
        return "⭐ NOVEL DISCOVERY - Extreme/Detached".to_string();
    }
    if avg_e > 0.6 && avg_a < 50.0 {
        return "⭐ NOVEL DISCOVERY - High Eccentricity".to_string();
    }
    "⚪ Unclassified".to_string()
}

/// Pre-loaded Kuiper Belt object data from NASA/JPL SBDB
pub fn get_kbo_data() -> Vec<KBO> {
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

        // Plutinos (3:2 Resonance)
        KBO { name: "15810 Arawn".into(), a: 39.21, e: 0.1141, i: 3.81, q: 34.734, ad: 43.68 },
        KBO { name: "28978 Ixion".into(), a: 39.35, e: 0.2442, i: 19.67, q: 29.740, ad: 48.96 },
        KBO { name: "38628 Huya".into(), a: 39.21, e: 0.2729, i: 15.48, q: 28.513, ad: 49.91 },
        KBO { name: "47171 Lempo".into(), a: 39.72, e: 0.2298, i: 8.40, q: 30.591, ad: 48.85 },
        KBO { name: "208996 Achlys".into(), a: 39.63, e: 0.1748, i: 13.55, q: 32.699, ad: 46.56 },
        KBO { name: "84922 (2003 VS2)".into(), a: 39.71, e: 0.0816, i: 14.76, q: 36.476, ad: 42.95 },
        KBO { name: "455502 (2003 UZ413)".into(), a: 39.43, e: 0.2182, i: 12.04, q: 30.824, ad: 48.03 },
        KBO { name: "15788 (1993 SB)".into(), a: 39.73, e: 0.3267, i: 1.94, q: 26.754, ad: 52.71 },
        KBO { name: "15789 (1993 SC)".into(), a: 39.74, e: 0.1839, i: 5.16, q: 32.433, ad: 47.05 },

        // Classical Belt
        KBO { name: "15760 Albion".into(), a: 44.2, e: 0.0725, i: 2.19, q: 40.995, ad: 47.40 },
        KBO { name: "20000 Varuna".into(), a: 43.18, e: 0.0525, i: 17.14, q: 40.909, ad: 45.45 },
        KBO { name: "19521 Chaos".into(), a: 46.11, e: 0.1105, i: 12.02, q: 41.013, ad: 51.20 },
        KBO { name: "79360 Sila-Nunam".into(), a: 44.04, e: 0.0141, i: 2.24, q: 43.415, ad: 44.66 },
        KBO { name: "66652 Borasisi".into(), a: 43.79, e: 0.0849, i: 0.56, q: 40.075, ad: 47.51 },
        KBO { name: "58534 Logos".into(), a: 45.23, e: 0.1227, i: 2.90, q: 39.681, ad: 50.79 },
        KBO { name: "88611 Teharonhiawako".into(), a: 44.05, e: 0.0266, i: 2.58, q: 42.877, ad: 45.22 },
        KBO { name: "53311 Deucalion".into(), a: 43.89, e: 0.0588, i: 0.37, q: 41.305, ad: 46.47 },
        KBO { name: "120347 Salacia".into(), a: 42.11, e: 0.1034, i: 23.93, q: 37.761, ad: 46.47 },
        KBO { name: "145452 Ritona".into(), a: 41.55, e: 0.0239, i: 19.26, q: 40.561, ad: 42.55 },
        KBO { name: "55565 Aya".into(), a: 47.3, e: 0.1277, i: 24.34, q: 41.259, ad: 53.34 },
        KBO { name: "174567 Varda".into(), a: 45.54, e: 0.1430, i: 21.51, q: 39.026, ad: 52.05 },
        KBO { name: "420356 Praamzius".into(), a: 42.89, e: 0.0044, i: 1.09, q: 42.699, ad: 43.08 },
        KBO { name: "148780 Altjira".into(), a: 44.54, e: 0.0568, i: 5.20, q: 42.009, ad: 47.07 },
        KBO { name: "385446 Manwe".into(), a: 43.84, e: 0.1164, i: 2.67, q: 38.733, ad: 48.94 },

        // Twotinos (2:1)
        KBO { name: "20161 (1996 TR66)".into(), a: 47.98, e: 0.3957, i: 12.43, q: 28.993, ad: 66.97 },
        KBO { name: "119979 (2002 WC19)".into(), a: 48.28, e: 0.2662, i: 9.16, q: 35.427, ad: 61.14 },

        // Scattered Disk
        KBO { name: "15874 (1996 TL66)".into(), a: 84.89, e: 0.5866, i: 23.96, q: 35.094, ad: 134.69 },
        KBO { name: "26181 (1996 GQ21)".into(), a: 92.48, e: 0.5874, i: 13.36, q: 38.152, ad: 146.81 },
        KBO { name: "26375 (1999 DE9)".into(), a: 55.5, e: 0.4201, i: 7.61, q: 32.184, ad: 78.81 },
        KBO { name: "84522 (2002 TC302)".into(), a: 55.84, e: 0.2995, i: 35.01, q: 39.113, ad: 72.56 },
        KBO { name: "145480 (2005 TB190)".into(), a: 75.93, e: 0.3912, i: 26.48, q: 46.227, ad: 105.64 },
        KBO { name: "229762 G!kun||'homdima".into(), a: 74.59, e: 0.4961, i: 23.33, q: 37.585, ad: 111.59 },
        KBO { name: "145451 Rumina".into(), a: 92.27, e: 0.6190, i: 28.70, q: 35.160, ad: 149.39 },

        // Extreme/Detached
        KBO { name: "148209 (2000 CR105)".into(), a: 228.7, e: 0.8071, i: 22.71, q: 44.117, ad: 413.29 },
        KBO { name: "82158 (2001 FP185)".into(), a: 213.4, e: 0.8398, i: 30.80, q: 34.190, ad: 392.66 },
        KBO { name: "87269 (2000 OO67)".into(), a: 617.9, e: 0.9663, i: 20.05, q: 20.850, ad: 1215.04 },
        KBO { name: "308933 (2006 SQ372)".into(), a: 839.3, e: 0.9711, i: 19.46, q: 24.226, ad: 1654.33 },
        KBO { name: "445473 (2010 VZ98)".into(), a: 159.8, e: 0.7851, i: 4.51, q: 34.356, ad: 285.32 },
        KBO { name: "303775 (2005 QU182)".into(), a: 112.2, e: 0.6696, i: 14.01, q: 37.059, ad: 187.28 },

        // High Inclination
        KBO { name: "65407 (2002 RP120)".into(), a: 54.53, e: 0.9542, i: 119.37, q: 2.498, ad: 106.57 },
        KBO { name: "127546 (2002 XU93)".into(), a: 66.9, e: 0.6862, i: 77.95, q: 20.991, ad: 112.80 },
        KBO { name: "336756 (2010 NV1)".into(), a: 305.2, e: 0.9690, i: 140.82, q: 9.457, ad: 600.93 },
        KBO { name: "418993 (2009 MS9)".into(), a: 375.7, e: 0.9706, i: 67.96, q: 11.046, ad: 740.43 },

        // Additional objects
        KBO { name: "15807 (1994 GV9)".into(), a: 43.66, e: 0.0635, i: 0.57, q: 40.891, ad: 46.43 },
        KBO { name: "33001 (1997 CU29)".into(), a: 43.63, e: 0.0346, i: 1.45, q: 42.117, ad: 45.14 },
        KBO { name: "55636 (2002 TX300)".into(), a: 43.48, e: 0.1213, i: 25.86, q: 38.204, ad: 48.75 },
        KBO { name: "55637 Uni".into(), a: 42.97, e: 0.1464, i: 19.41, q: 36.683, ad: 49.27 },
        KBO { name: "42355 Typhon".into(), a: 37.71, e: 0.5367, i: 2.43, q: 17.470, ad: 57.94 },
        KBO { name: "65489 Ceto".into(), a: 100.5, e: 0.8238, i: 22.30, q: 17.709, ad: 183.25 },
        KBO { name: "307261 Mani".into(), a: 41.6, e: 0.1487, i: 17.70, q: 35.410, ad: 47.78 },
        KBO { name: "202421 (2005 UQ513)".into(), a: 43.53, e: 0.1452, i: 25.72, q: 37.207, ad: 49.85 },
        KBO { name: "120178 (2003 OP32)".into(), a: 43.18, e: 0.1034, i: 27.15, q: 38.710, ad: 47.64 },
        KBO { name: "119951 (2002 KX14)".into(), a: 38.62, e: 0.0448, i: 0.41, q: 36.893, ad: 40.35 },
        KBO { name: "90568 Goibniu".into(), a: 41.81, e: 0.0760, i: 22.04, q: 38.631, ad: 44.99 },
        KBO { name: "305543 (2008 QY40)".into(), a: 62.37, e: 0.4097, i: 25.12, q: 36.816, ad: 87.91 },
        KBO { name: "353222 (2009 YD7)".into(), a: 125.7, e: 0.8936, i: 30.77, q: 13.379, ad: 238.05 },
        KBO { name: "451657 (2012 WD36)".into(), a: 77.71, e: 0.5172, i: 23.68, q: 37.517, ad: 117.90 },
        KBO { name: "437360 (2013 TV158)".into(), a: 114.1, e: 0.6801, i: 31.14, q: 36.482, ad: 191.62 },
        KBO { name: "386723 (2009 YE7)".into(), a: 44.59, e: 0.1373, i: 29.07, q: 38.472, ad: 50.71 },
        KBO { name: "308193 (2005 CB79)".into(), a: 43.45, e: 0.1428, i: 28.60, q: 37.243, ad: 49.65 },
        KBO { name: "444030 (2004 NT33)".into(), a: 43.42, e: 0.1485, i: 31.21, q: 36.966, ad: 49.86 },
        KBO { name: "230965 (2004 XA192)".into(), a: 47.5, e: 0.2533, i: 38.08, q: 35.472, ad: 59.53 },
        KBO { name: "315530 (2008 AP129)".into(), a: 41.99, e: 0.1427, i: 27.41, q: 36.003, ad: 47.99 },
        KBO { name: "341520 Mors-Somnus".into(), a: 39.56, e: 0.2702, i: 11.27, q: 28.871, ad: 50.25 },
        KBO { name: "78799 Xewioso".into(), a: 37.69, e: 0.2435, i: 14.34, q: 28.511, ad: 46.86 },
    ]
}

fn main() {
    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║   KUIPER BELT DENSITY-BASED CLUSTERING ANALYSIS (DBSCAN)     ║");
    println!("║                  Data Source: NASA/JPL SBDB                  ║");
    println!("╚══════════════════════════════════════════════════════════════╝\n");

    let objects = get_kbo_data();
    println!("📥 Loaded {} Trans-Neptunian Objects\n", objects.len());

    // Population statistics
    let plutinos: Vec<_> = objects.iter().filter(|o| (o.a - 39.4).abs() < 2.0 && o.e < 0.4).collect();
    let classical: Vec<_> = objects.iter().filter(|o| o.a >= 42.0 && o.a <= 48.0 && o.e < 0.2).collect();
    let scattered: Vec<_> = objects.iter().filter(|o| o.a > 50.0 && o.e > 0.3).collect();

    println!("📊 Population Overview:");
    println!("   Plutinos (3:2 resonance):    {}", plutinos.len());
    println!("   Classical Belt:              {}", classical.len());
    println!("   Scattered/Detached:          {}", scattered.len());
    println!();

    // Run DBSCAN with multiple parameter combinations
    println!("🔍 Running DBSCAN Parameter Search...\n");

    let param_combos = vec![
        (0.08, 2), (0.10, 2), (0.12, 2), (0.15, 3),
        (0.18, 3), (0.20, 3), (0.25, 4),
    ];

    let mut best_result = None;
    let mut best_score = 0.0;
    let mut best_params = (0.0, 0);

    for (eps, min_pts) in &param_combos {
        let result = dbscan(&objects, *eps, *min_pts);

        // Score based on clustering quality
        let clustered = objects.len() - result.noise.len();
        let ratio = clustered as f32 / objects.len() as f32;
        let cluster_count_score = if result.clusters.len() >= 5 && result.clusters.len() <= 15 { 0.3 } else { 0.1 };
        let known_resonances = result.cluster_stats.iter()
            .filter(|s| s.classification.contains("Resonance") || s.classification.contains("Classical"))
            .count();
        let score = ratio * 0.4 + cluster_count_score + known_resonances as f32 * 0.1;

        if score > best_score {
            best_score = score;
            best_result = Some(result);
            best_params = (*eps, *min_pts);
        }
    }

    let result = best_result.unwrap();

    println!("═══════════════════════════════════════════════════════════════");
    println!("                      CLUSTERING RESULTS                       ");
    println!("═══════════════════════════════════════════════════════════════\n");

    println!("📈 Optimal Parameters:");
    println!("   Epsilon:    {:.2}", best_params.0);
    println!("   Min Points: {}", best_params.1);
    println!();

    println!("📊 Clustering Statistics:");
    println!("   Total objects:      {}", objects.len());
    println!("   Clusters found:     {}", result.clusters.len());
    println!("   Clustered objects:  {}", objects.len() - result.noise.len());
    println!("   Noise objects:      {}", result.noise.len());
    println!("   Clustering ratio:   {:.1}%", (objects.len() - result.noise.len()) as f32 / objects.len() as f32 * 100.0);
    println!();

    println!("🔭 Cluster Classifications:");
    println!("───────────────────────────────────────────────────────────────");

    let mut sorted_stats = result.cluster_stats.clone();
    sorted_stats.sort_by(|a, b| b.count.cmp(&a.count));

    for stat in &sorted_stats {
        println!(
            "   Cluster {:2}: {:3} objects | a={:6.1} AU | e={:.2} | i={:5.1}°",
            stat.id, stat.count, stat.avg_a, stat.avg_e, stat.avg_i
        );
        println!("              {}", stat.classification);
        println!();
    }

    // Novel discoveries
    let novel: Vec<_> = sorted_stats.iter()
        .filter(|s| s.classification.contains("NOVEL"))
        .collect();

    if !novel.is_empty() {
        println!("═══════════════════════════════════════════════════════════════");
        println!("                    ⭐ NOVEL DISCOVERIES ⭐                     ");
        println!("═══════════════════════════════════════════════════════════════\n");

        for stat in novel {
            println!("   {} - {} objects", stat.classification, stat.count);
            println!("   Average orbital parameters:");
            println!("     Semi-major axis: {:.1} AU", stat.avg_a);
            println!("     Eccentricity:    {:.3}", stat.avg_e);
            println!("     Inclination:     {:.1}°", stat.avg_i);
            println!();

            // Find members
            if let Some(cluster) = result.clusters.iter().find(|c| {
                let member_objects: Vec<&KBO> = c.iter()
                    .filter_map(|name| objects.iter().find(|o| &o.name == name))
                    .collect();
                if member_objects.is_empty() { return false; }
                let avg_a = member_objects.iter().map(|o| o.a).sum::<f32>() / member_objects.len() as f32;
                (avg_a - stat.avg_a).abs() < 1.0
            }) {
                println!("   Members:");
                for member in cluster.iter().take(10) {
                    if let Some(obj) = objects.iter().find(|o| &o.name == member) {
                        println!("     • {} (a={:.1}, e={:.2}, i={:.1}°)", obj.name, obj.a, obj.e, obj.i);
                    }
                }
                if cluster.len() > 10 {
                    println!("     ... and {} more", cluster.len() - 10);
                }
            }
            println!();
        }

        println!("   Recommended Follow-up:");
        println!("   • Numerical orbit integration to confirm dynamical coherence");
        println!("   • Check for common proper orbital elements");
        println!("   • Spectroscopic observations for compositional analysis");
        println!("   • Search for additional members in survey data");
    }

    // Extreme objects
    println!("\n═══════════════════════════════════════════════════════════════");
    println!("                     EXTREME OBJECTS                           ");
    println!("═══════════════════════════════════════════════════════════════\n");

    let mut most_distant = objects.clone();
    most_distant.sort_by(|a, b| b.a.partial_cmp(&a.a).unwrap());

    println!("   Most Distant (by semi-major axis):");
    for obj in most_distant.iter().take(5) {
        println!("     • {} - a={:.1} AU, e={:.3}", obj.name, obj.a, obj.e);
    }

    let mut highest_e = objects.clone();
    highest_e.sort_by(|a, b| b.e.partial_cmp(&a.e).unwrap());

    println!("\n   Most Eccentric:");
    for obj in highest_e.iter().take(5) {
        println!("     • {} - e={:.3}, a={:.1} AU", obj.name, obj.e, obj.a);
    }

    let mut highest_i = objects.clone();
    highest_i.sort_by(|a, b| b.i.partial_cmp(&a.i).unwrap());

    println!("\n   Highest Inclination:");
    for obj in highest_i.iter().take(5) {
        println!("     • {} - i={:.1}°, a={:.1} AU", obj.name, obj.i, obj.a);
    }

    // Planet Nine candidates
    let p9_candidates: Vec<_> = objects.iter()
        .filter(|o| o.a > 250.0 && o.q > 30.0)
        .collect();

    println!("\n   Potential Planet Nine Influenced (a>250 AU, q>30 AU):");
    if p9_candidates.is_empty() {
        println!("     None found with these criteria");
    } else {
        for obj in p9_candidates {
            println!("     • {} - a={:.1} AU, q={:.1} AU", obj.name, obj.a, obj.q);
        }
    }

    println!("\n═══════════════════════════════════════════════════════════════");
    println!("                      ANALYSIS COMPLETE                        ");
    println!("   Data source: NASA/JPL Small-Body Database (ssd.jpl.nasa.gov)");
    println!("═══════════════════════════════════════════════════════════════\n");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dbscan_clusters() {
        let objects = get_kbo_data();
        let result = dbscan(&objects, 0.15, 3);

        assert!(!result.clusters.is_empty());
        assert!(result.clusters.len() >= 3); // Should find at least Plutinos, Classical, and Scattered
    }

    #[test]
    fn test_classification() {
        assert!(classify_cluster(39.4, 0.25, 15.0).contains("Plutino"));
        assert!(classify_cluster(43.5, 0.05, 5.0).contains("Classical"));
        assert!(classify_cluster(80.0, 0.5, 20.0).contains("Scattered"));
    }

    #[test]
    fn test_tisserand() {
        let pluto = KBO { name: "Pluto".into(), a: 39.59, e: 0.2518, i: 17.15, q: 29.619, ad: 49.56 };
        let t = pluto.tisserand();
        assert!(t > 2.0 && t < 4.0);
    }
}
