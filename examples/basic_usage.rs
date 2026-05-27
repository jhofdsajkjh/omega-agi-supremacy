//! OMEGA AGI Basic Usage Example
//! 
//! Demonstrates core functionality of the OMEGA AGI system.

use omega_agi::hypercore::{OmegaAGI, Config};
use omega_agi::engineering::{CodeGenerator, TestHarness};
use omega_agi::optimization::PerformanceOptimizer;

fn main() {
    println!("🌀 OMEGA AGI Supremacy - Basic Example");
    println!("=====================================\n");

    // Initialize configuration
    let config = Config::default()
        .with_github_token("your_github_token_here")
        .with_log_level("info");

    // Create OMEGA AGI instance
    let omega = OmegaAGI::new(config).expect("Failed to initialize OMEGA AGI");

    println!("✅ OMEGA AGI initialized successfully");
    println!("   Version: {}", omega.version());
    println!("   Layers: {}", omega.layer_count());

    // Example: Code generation
    let generator = CodeGenerator::new();
    let code = generator.generate("python", "Hello World program");
    println!("\n📝 Generated Code:\n{}", code);

    // Example: Test execution
    let harness = TestHarness::new();
    println!("\n🧪 Running tests...");
    let results = harness.run_all();
    println!("   Tests passed: {}/{}", results.passed(), results.total());

    // Example: Performance optimization
    let optimizer = PerformanceOptimizer::new();
    println!("\n⚡ Running optimizations...");
    let report = optimizer.optimize_all();
    println!("   Optimizations: {}", report.strategy_count());

    println!("\n✨ OMEGA AGI Basic Example Complete!");
}