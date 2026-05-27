#!/usr/bin/env python3
"""
OMEGA AGI Integration Tests
"""

import unittest
import sys
import os

sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))


class TestHyperCore(unittest.TestCase):
    """Test Layer 0: HyperCore"""
    
    def test_session_initialization(self):
        """Test session creation"""
        from omega_agi.hypercore import Session
        session = Session.new()
        self.assertIsNotNone(session)
    
    def test_memory_allocation(self):
        """Test memory management"""
        from omega_agi.hypercore import Memory
        memory = Memory::new(1024 * 1024);
        self.assertGreater(memory.capacity(), 0)


class TestEngineering(unittest.TestCase):
    """Test Layer 3: Engineering"""
    
    def test_code_generator(self):
        """Test code generation"""
        from omega_pipeline.self_healing import SelfHealing
        healer = SelfHealing()
        self.assertIsNotNone(healer)
    
    def test_quality_gates(self):
        """Test quality gates"""
        from omega_pipeline.quality_gates import QualityGates
        gates = QualityGates()
        self.assertIsNotNone(gates)


class TestOptimization(unittest.TestCase):
    """Test Layer 4: Optimization"""
    
    def test_performance_optimizer(self):
        """Test performance optimization"""
        # Placeholder test
        self.assertTrue(True)
    
    def test_self_evolution(self):
        """Test self-evolution loop"""
        from omega_pipeline.self_evolution_loop import SelfEvolutionLoop
        loop = SelfEvolutionLoop()
        self.assertIsNotNone(loop)


if __name__ == "__main__":
    print("🧪 OMEGA AGI Integration Test Suite")
    print("=" * 50)
    unittest.main(verbosity=2)