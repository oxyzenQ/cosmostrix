// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Visual tuning — auto-find parameters that match target entropy/gini.
//!
//! Uses iterative heuristic search: run short benchmark, measure visual
//! metrics, adjust density/glitch, repeat. Max 10 iterations.

use crate::app::CloudConfig;
use crate::config::GlitchLevel;

/// Target values for visual tuning.
#[derive(Debug, Clone, Default)]
pub struct TuneTarget {
    pub entropy: Option<f64>,
    pub gini: Option<f64>,
}

/// Result of a tuning run.
#[derive(Debug, Clone)]
pub struct TuneResult {
    pub best_density: f32,
    pub best_speed: f32,
    pub best_glitch_level: GlitchLevel,
    pub achieved_entropy: f64,
    pub achieved_gini: f64,
    pub entropy_error_pct: Option<f64>,
    pub gini_error_pct: Option<f64>,
    pub iterations: u32,
}

/// Parse --tune-visual target string (e.g. "entropy=5.2,gini=0.6").
pub fn parse_tune_target(input: &str) -> Result<TuneTarget, String> {
    let mut target = TuneTarget::default();
    for part in input.split(',') {
        let part = part.trim();
        if let Some((key, val)) = part.split_once('=') {
            let val: f64 = val
                .trim()
                .parse()
                .map_err(|_| format!("error: --tune-visual invalid value for '{key}': '{val}'"))?;
            match key.trim() {
                "entropy" => target.entropy = Some(val),
                "gini" => target.gini = Some(val),
                other => {
                    return Err(format!(
                        "error: --tune-visual unknown target '{other}' (use entropy=, gini=)"
                    ));
                }
            }
        } else {
            return Err(format!(
                "error: --tune-visual '{part}' missing '=' (use format: entropy=5.2,gini=0.6)"
            ));
        }
    }
    if target.entropy.is_none() && target.gini.is_none() {
        return Err("error: --tune-visual requires at least one target (entropy=, gini=)".into());
    }
    Ok(target)
}

/// Run auto-tuning: iteratively adjust parameters to match target visual metrics.
pub fn auto_tune(
    config: &CloudConfig,
    target: &TuneTarget,
    duration_secs: u64,
) -> std::io::Result<TuneResult> {
    let max_iterations = 10u32;
    let density_step = 0.05f32;
    let min_density = 0.1f32;
    let max_density = 5.0f32;

    let glitch_levels = [
        GlitchLevel::None,
        GlitchLevel::Subtle,
        GlitchLevel::Default,
        GlitchLevel::Intense,
    ];

    let mut best_result: Option<TuneResult> = None;
    let mut best_error = f64::MAX;

    let mut current_density = config.base_density;
    let mut current_glitch_idx: usize = match config.glitch_pct {
        p if p < 0.01 => 0, // None
        p if p < 5.0 => 1,  // Subtle
        p if p < 15.0 => 2, // Default
        _ => 3,             // Intense
    };

    eprintln!("[tune-visual] Starting visual tuning (max {max_iterations} iterations, {duration_secs}s each)...");
    eprintln!(
        "[tune-visual] Target: entropy={:?}, gini={:?}",
        target.entropy, target.gini
    );
    eprintln!();

    for iteration in 1..=max_iterations {
        // Create config with current parameters
        let mut tune_cfg = config.clone_config();
        tune_cfg.screen_size = config.screen_size.or(Some((120, 40)));
        tune_cfg.base_density = current_density;

        // Run benchmark capture
        let data = crate::bench::run_benchmark_capture(&tune_cfg, duration_secs)?;

        let entropy = data
            .visual
            .as_ref()
            .map(|v| v.frame_entropy_bits)
            .unwrap_or(0.0);
        let gini = data.visual.as_ref().map(|v| v.density_gini).unwrap_or(0.0);

        // Compute error (sum of squared relative errors)
        let entropy_err = target.entropy.map(|t| {
            if t > 0.0 {
                ((entropy - t) / t).abs() * 100.0
            } else {
                0.0
            }
        });
        let gini_err = target.gini.map(|t| {
            if t > 0.0 {
                ((gini - t) / t).abs() * 100.0
            } else {
                0.0
            }
        });

        let total_error: f64 = [entropy_err, gini_err]
            .iter()
            .filter_map(|e| *e)
            .map(|e| e * e)
            .sum();

        eprintln!(
            "[tune-visual] iter {iteration}/{max_iterations}: density={current_density:.2} glitch={:?} → entropy={entropy:.2} gini={gini:.4} error={total_error:.2}",
            glitch_levels[current_glitch_idx]
        );

        if total_error < best_error {
            best_error = total_error;
            best_result = Some(TuneResult {
                best_density: current_density,
                best_speed: config.speed,
                best_glitch_level: glitch_levels[current_glitch_idx],
                achieved_entropy: entropy,
                achieved_gini: gini,
                entropy_error_pct: entropy_err,
                gini_error_pct: gini_err,
                iterations: iteration,
            });
        }

        // Check if we're close enough
        if total_error < 1.0 {
            eprintln!("[tune-visual] Converged (error < 1%)");
            break;
        }

        // Adjust parameters
        let need_higher_entropy = target.entropy.is_some_and(|t| entropy < t);
        let need_lower_entropy = target.entropy.is_some_and(|t| entropy > t);
        let need_lower_gini = target.gini.is_some_and(|g| gini > g);
        let need_higher_gini = target.gini.is_some_and(|g| gini < g);

        // Heuristic: density increases entropy, decreases gini (more uniform)
        if need_higher_entropy || need_lower_gini {
            current_density = (current_density + density_step).min(max_density);
        } else if need_lower_entropy || need_higher_gini {
            current_density = (current_density - density_step).max(min_density);
        }

        // If density is at extremes, try changing glitch level
        if current_density >= max_density && need_higher_entropy {
            current_glitch_idx = (current_glitch_idx + 1).min(3);
        }
        if current_density <= min_density && need_lower_entropy {
            current_glitch_idx = current_glitch_idx.saturating_sub(1);
        }
    }

    let result = best_result.unwrap_or(TuneResult {
        best_density: config.base_density,
        best_speed: config.speed,
        best_glitch_level: glitch_levels[current_glitch_idx],
        achieved_entropy: 0.0,
        achieved_gini: 0.0,
        entropy_error_pct: None,
        gini_error_pct: None,
        iterations: 0,
    });

    // Print result
    println!("VISUAL TUNING RESULT");
    println!("────────────────────");
    if let Some(t) = target.entropy {
        println!(
            "  Target entropy:  {t:.2} (achieved: {:.2}, error: {:.1}%)",
            result.achieved_entropy,
            result.entropy_error_pct.unwrap_or(0.0)
        );
    }
    if let Some(t) = target.gini {
        println!(
            "  Target gini:     {t:.4} (achieved: {:.4}, error: {:.1}%)",
            result.achieved_gini,
            result.gini_error_pct.unwrap_or(0.0)
        );
    }
    println!();
    println!("  Recommended parameters:");
    println!("    density:       {:.2}", result.best_density);
    println!("    speed:         {:.1}", result.best_speed);
    println!("    glitch-level:  {:?}", result.best_glitch_level);
    println!();
    println!("  Config snippet (add to ~/.config/cosmostrix/config.toml):");
    println!("    density = {:.2}", result.best_density);
    println!("    speed = {:.1}", result.best_speed);
    println!("    glitch-level = \"{:?}\"", result.best_glitch_level);
    println!();
    println!("  Iterations: {}", result.iterations);
    println!();

    Ok(result)
}
