#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
Apex AGI Memory System
Experience-RAG: 检索增强 + 长期经验沉淀
Reflexion: 自我反思与改进追踪
"""

import json, datetime, os
from collections import defaultdict

class ExperienceRAG:
    """经验驱动的 RAG 框架"""
    def __init__(self, memory_path):
        self.memory_path = memory_path
        self.experience_index = []
        self.vectors = defaultdict(list)
    
    def store(self, experience):
        """存储经验"""
        entry = {
            "id": len(self.experience_index),
            "timestamp": datetime.datetime.now().isoformat(),
            "content": experience.get("content", ""),
            "type": experience.get("type", "general"),
            "tags": experience.get("tags", []),
            "outcomes": experience.get("outcomes", {}),
            "genes_used": experience.get("genes_used", [])
        }
        self.experience_index.append(entry)
        self._update_vectors(entry)
        return entry["id"]
    
    def _update_vectors(self, entry):
        """基于内容关键词构建简单向量索引"""
        for tag in entry["tags"]:
            self.vectors[tag].append(entry["id"])
    
    def retrieve(self, query, top_k=5):
        """检索相关经验"""
        query_tags = [w for w in query.lower().split() if len(w) > 3]
        candidates = set()
        for tag in query_tags:
            candidates.update(self.vectors.get(tag, []))
        
        results = [self.experience_index[i] for i in list(candidates)[:top_k]]
        return results
    
    def save(self):
        """持久化到磁盘"""
        os.makedirs(os.path.dirname(self.memory_path), exist_ok=True)
        with open(self.memory_path, 'w', encoding='utf-8') as f:
            json.dump(self.experience_index, f, ensure_ascii=False, indent=2)

class Reflexion:
    """言语强化学习框架"""
    def __init__(self):
        self.reflection_log = []
        self.improvement_history = []
    
    def reflect(self, action_result, expected, actual):
        """反思行动结果"""
        delta = {
            "timestamp": datetime.datetime.now().isoformat(),
            "action": action_result,
            "expected": expected,
            "actual": actual,
            "gap": self._calc_gap(expected, actual),
            "verbal_feedback": self._generate_feedback(expected, actual)
        }
        self.reflection_log.append(delta)
        return delta
    
    def _calc_gap(self, expected, actual):
        if isinstance(expected, (int, float)) and isinstance(actual, (int, float)):
            return round((actual - expected) / max(expected, 1), 3)
        return 0
    
    def _generate_feedback(self, expected, actual):
        gap = self._calc_gap(expected, actual)
        if gap > 0:
            return f"Exceeded target by {gap*100:.1f}%"
        elif gap < 0:
            return f"Missed target by {abs(gap)*100:.1f}%, need improvement"
        return "Met target exactly"
    
    def get_insights(self):
        """从反思中提取洞察"""
        avg_gap = sum(d["gap"] for d in self.reflection_log) / max(len(self.reflection_log), 1)
        return {
            "total_reflections": len(self.reflection_log),
            "avg_gap": round(avg_gap, 3),
            "improvements": self.improvement_history
        }

if __name__ == "__main__":
    rag = ExperienceRAG(r"C:\Users\Administrator\ApexSpiral\agi-system\memory\experience_rag.json")
    ref = Reflexion()
    
    # 模拟存储经验
    rag.store({"content": "Resolved SSL EOF error by switching HTTPS context", "type": "debug", "tags": ["ssl", "network", "error"], "genes_used": ["gene_001"]})
    
    results = rag.retrieve("SSL network error", top_k=3)
    print(f"Retrieved {len(results)} experiences")
    
    insight = ref.reflect("Executed gene scan", 10, 8)
    print(f"Reflexion: {insight['verbal_feedback']}")
