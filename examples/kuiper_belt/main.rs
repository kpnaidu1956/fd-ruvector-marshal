//! # Kuiper Belt Self-Learning Density Clustering Analysis
//!
//! This program performs DBSCAN clustering on Trans-Neptunian Objects (TNOs)
//! using NASA/JPL Small-Body Database data. It leverages ruvector's AgenticDB
//! for self-learning pattern discovery and identifies potential novel discoveries.
//!
//! ## Usage
//! ```bash
//! cargo run --example kuiper_belt_main --features storage
//! ```
//!
//! ## Data Source
//! NASA/JPL Small-Body Database Query API:
//! https://ssd-api.jpl.nasa.gov/sbdb_query.api

mod kuiper_cluster;
mod kbo_data;

use kuiper_cluster::{
    DBSCANClusterer, SelfLearningAnalyzer, KuiperBeltObject, ClusterSignificance,
};
use kbo_data::get_kbo_data;
use ruvector_core::advanced::tda::TopologicalAnalyzer;

fn main() -> ruvector_core::Result<()> {
    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║   KUIPER BELT DENSITY-BASED CLUSTERING WITH SELF-LEARNING    ║");
    println!("║                  Powered by RuVector AgenticDB               ║");
    println!("╚══════════════════════════════════════════════════════════════╝\n");

    // Load Kuiper Belt objects data
    println!("📥 Loading Kuiper Belt Objects from NASA/JPL SBDB...\n");
    let objects = get_kbo_data();
    println!("   Loaded {} Trans-Neptunian Objects\n", objects.len());

    // Display population statistics
    display_population_stats(&objects);

    // Initialize self-learning analyzer
    println!("\n🧠 Initializing Self-Learning Analyzer...\n");
    let mut analyzer = SelfLearningAnalyzer::new("./kuiper_analysis.db")?;

    // Run analysis with learning
    println!("🔍 Running DBSCAN Clustering with Parameter Optimization...\n");
    let result = analyzer.analyze_with_learning(&objects, 3)?;

    // Display results
    println!("{}", result.summary());

    // Perform additional topological analysis
    println!("\n🔬 Additional Topological Data Analysis...\n");
    perform_tda_analysis(&objects)?;

    // Display resonance analysis
    println!("\n📊 Mean-Motion Resonance Analysis...\n");
    analyze_resonances(&objects);

    // Display extreme objects
    println!("\n🌟 Extreme Objects of Interest...\n");
    find_extreme_objects(&objects);

    // Interactive cluster exploration (simulated)
    println!("\n🔭 DISCOVERY SUMMARY\n");
    println!("───────────────────────────────────────────────────────────────");

    let novel = result.clustering.novel_discoveries();
    if !novel.is_empty() {
        println!("   {} potential novel dynamical groupings discovered!", novel.len());
        println!("\n   These clusters may represent:");
        println!("   • Previously unidentified collisional families");
        println!("   • New mean-motion resonance populations");
        println!("   • Dynamically coherent sub-populations");
        println!("\n   Recommended follow-up:");
        println!("   • Numerical orbit integration to confirm dynamical coherence");
        println!("   • Spectroscopic observations to check compositional similarity");
        println!("   • Examination of Tisserand parameters and proper elements");
    } else {
        println!("   No novel clusters identified in this analysis.");
        println!("   Consider adjusting clustering parameters or adding more data.");
    }

    println!("\n═══════════════════════════════════════════════════════════════");
    println!("              Analysis Complete - Data Saved to DB             ");
    println!("═══════════════════════════════════════════════════════════════\n");

    Ok(())
}

fn display_population_stats(objects: &[KuiperBeltObject]) {
    println!("📈 Population Statistics:");
    println!("───────────────────────────────────────────────────────────────");

    let plutinos: Vec<_> = objects.iter().filter(|o| o.is_plutino()).collect();
    let classical: Vec<_> = objects.iter().filter(|o| o.is_classical()).collect();
    let scattered: Vec<_> = objects.iter().filter(|o| o.is_scattered()).collect();
    let detached: Vec<_> = objects.iter().filter(|o| o.is_detached()).collect();

    let avg_a: f32 = objects.iter().map(|o| o.a).sum::<f32>() / objects.len() as f32;
    let avg_e: f32 = objects.iter().map(|o| o.e).sum::<f32>() / objects.len() as f32;
    let avg_i: f32 = objects.iter().map(|o| o.i).sum::<f32>() / objects.len() as f32;

    let max_a = objects.iter().map(|o| o.a).max_by(|a, b| a.partial_cmp(b).unwrap()).unwrap_or(0.0);
    let min_a = objects.iter().map(|o| o.a).min_by(|a, b| a.partial_cmp(b).unwrap()).unwrap_or(0.0);

    println!("   Plutinos (3:2 resonance):      {:4}", plutinos.len());
    println!("   Classical KBOs:                {:4}", classical.len());
    println!("   Scattered Disk Objects:        {:4}", scattered.len());
    println!("   Detached Objects:              {:4}", detached.len());
    println!();
    println!("   Orbital Parameter Ranges:");
    println!("     Semi-major axis (a):  {:.1} - {:.1} AU (avg: {:.1} AU)", min_a, max_a, avg_a);
    println!("     Eccentricity (e):     0 - 1 (avg: {:.3})", avg_e);
    println!("     Inclination (i):      0° - 90° (avg: {:.1}°)", avg_i);
}

fn perform_tda_analysis(objects: &[KuiperBeltObject]) -> ruvector_core::Result<()> {
    let features: Vec<Vec<f32>> = objects.iter()
        .map(|o| o.to_feature_vector())
        .collect();

    // Multiple scales for persistence analysis
    let scales = vec![0.05, 0.10, 0.15, 0.20, 0.30];

    println!("   Multi-scale Topological Analysis:");
    println!("   ──────────────────────────────────────────────────────────");
    println!("   {:<10} {:<12} {:<12} {:<12}", "Epsilon", "Components", "Clustering", "Quality");
    println!("   ──────────────────────────────────────────────────────────");

    for &eps in &scales {
        let tda = TopologicalAnalyzer::new(10, eps);
        let quality = tda.analyze(&features)?;

        println!("   {:<10.3} {:<12} {:<12.3} {:<12.3}",
            eps, quality.connected_components, quality.clustering_coefficient, quality.quality_score);
    }

    println!("\n   Interpretation:");
    println!("   • Lower epsilon reveals finer structure (more components)");
    println!("   • Higher clustering coefficient indicates tight groupings");
    println!("   • Quality score balances separation and coherence");

    Ok(())
}

fn analyze_resonances(objects: &[KuiperBeltObject]) {
    // Known Neptune mean-motion resonances and their semi-major axes
    let resonances = vec![
        ("4:3", 36.5),
        ("3:2", 39.4),  // Plutinos
        ("5:3", 42.3),
        ("7:4", 43.7),
        ("2:1", 47.8),  // Twotinos
        ("5:2", 55.4),
        ("3:1", 62.6),
        ("4:1", 75.9),
        ("5:1", 87.9),
    ];

    println!("   Neptune Mean-Motion Resonances:");
    println!("   ──────────────────────────────────────────────────────────");
    println!("   {:<12} {:<12} {:<12}", "Resonance", "a (AU)", "Count");
    println!("   ──────────────────────────────────────────────────────────");

    for (name, a_res) in &resonances {
        let count = objects.iter()
            .filter(|o| (o.a - a_res).abs() < 1.5)
            .count();

        if count > 0 {
            println!("   {:<12} {:<12.1} {:<12}", name, a_res, count);
        }
    }

    println!("\n   Non-resonant populations:");
    let non_resonant: Vec<_> = objects.iter()
        .filter(|o| {
            !resonances.iter().any(|(_, a)| (o.a - a).abs() < 1.5) && o.a < 100.0
        })
        .collect();

    println!("   • {} objects not near major resonances", non_resonant.len());
    println!("   • These may occupy higher-order resonances or be unstable");
}

fn find_extreme_objects(objects: &[KuiperBeltObject]) {
    // Most distant (highest semi-major axis)
    let most_distant: Vec<_> = {
        let mut sorted: Vec<_> = objects.iter().collect();
        sorted.sort_by(|a, b| b.a.partial_cmp(&a.a).unwrap());
        sorted.into_iter().take(5).collect()
    };

    // Highest eccentricity
    let highest_e: Vec<_> = {
        let mut sorted: Vec<_> = objects.iter().collect();
        sorted.sort_by(|a, b| b.e.partial_cmp(&a.e).unwrap());
        sorted.into_iter().take(5).collect()
    };

    // Highest inclination
    let highest_i: Vec<_> = {
        let mut sorted: Vec<_> = objects.iter().collect();
        sorted.sort_by(|a, b| b.i.partial_cmp(&a.i).unwrap());
        sorted.into_iter().take(5).collect()
    };

    println!("   Most Distant Objects (highest semi-major axis):");
    for obj in &most_distant {
        println!("     • {} - a={:.1} AU, e={:.3}, i={:.1}°", obj.name, obj.a, obj.e, obj.i);
    }

    println!("\n   Most Eccentric Objects:");
    for obj in &highest_e {
        println!("     • {} - e={:.3}, a={:.1} AU", obj.name, obj.e, obj.a);
    }

    println!("\n   Highest Inclination Objects:");
    for obj in &highest_i {
        println!("     • {} - i={:.1}°, a={:.1} AU", obj.name, obj.i, obj.a);
    }

    // Objects with unusual Tisserand parameters
    println!("\n   Potential Planet Nine Influenced Objects (a > 250 AU, q > 30 AU):");
    let potential_p9: Vec<_> = objects.iter()
        .filter(|o| o.a > 250.0 && o.q > 30.0)
        .collect();

    if potential_p9.is_empty() {
        println!("     None found in current dataset");
    } else {
        for obj in potential_p9 {
            println!("     • {} - a={:.1} AU, q={:.1} AU, i={:.1}°", obj.name, obj.a, obj.q, obj.i);
        }
    }
}
