#!/usr/bin/env python3
"""
OMEGA AGI Evolution Example
Demonstrates self-evolution capabilities
"""

from omega_pipeline.self_evolution_loop import SelfEvolutionLoop

def main():
    print("🧬 OMEGA AGI - Self Evolution");
    print("=" * 50)

    # Initialize evolution loop
    evolution = SelfEvolutionLoop()

    print("\n🔄 Running evolution cycle...")
    result = evolution.run_cycle()

    print(f"\n📊 Evolution Results:")
    print(f"   ΔG (Growth): {result.get('delta_g', 0):.4f}")
    print(f"   Genes evolved: {result.get('genes_evolved', 0)}")
    print(f"   Fitness: {result.get('fitness', 0):.4f}")

    # APEX formula verification
    print("\n🧮 APEX Formula Check:")
    params = evolution.get_apex_parameters()
    print(f"   C (Complexity): {params.get('C', 0)}")
    print(f"   Λ (Learning): {params.get('Lambda', 0)}")
    print(f"   Ω (Organization): {params.get('Omega', 0)}")
    print(f"   Φ (Stability): {params.get('Phi', 0)}")
    print(f"   Ξ (Healing): {params.get('Xi', 0)}")

    print("\n✨ Evolution Cycle Complete!")

if __name__ == "__main__":
    main()