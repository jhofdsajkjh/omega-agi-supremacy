#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
Apex Gene Storage & Retrieval System
基于类型、来源、评分的多维索引
"""

import json, os, datetime
from collections import defaultdict

class GeneStorage:
    def __init__(self, gene_bank_path):
        self.gene_bank_path = gene_bank_path
        self.index = defaultdict(list)
        self.type_index = defaultdict(list)
        self.score_index = defaultdict(list)
    
    def load(self):
        """加载基因库并构建索引"""
        with open(self.gene_bank_path, 'r', encoding='utf-8') as f:
            genes = json.load(f)
        
        self.genes = genes
        self._build_indexes()
        return len(genes)
    
    def _build_indexes(self):
        """构建多维索引"""
        self.index.clear()
        self.type_index.clear()
        self.score_index.clear()
        
        for gene in self.genes:
            # 名称索引
            for word in gene.get("name", "").split():
                if len(word) > 2:
                    self.index[word].append(gene["id"])
            
            # 类型索引
            cat = gene.get("category", "unknown")
            self.type_index[cat].append(gene["id"])
            
            # 评分索引
            score = gene.get("apex_score", 0)
            bucket = int(score * 10) / 10
            self.score_index[bucket].append(gene["id"])
    
    def search(self, query, top_k=10):
        """多维检索"""
        query_words = [w for w in query.lower().split() if len(w) > 2]
        candidates = set()
        
        for word in query_words:
            candidates.update(self.index.get(word, []))
        
        # 如果没有精确匹配，按评分排序返回前 top_k
        if not candidates:
            sorted_genes = sorted(self.genes, key=lambda g: g.get("apex_score", 0), reverse=True)
            return sorted_genes[:top_k]
        
        return [g for g in self.genes if g["id"] in candidates][:top_k]
    
    def filter_by_type(self, gene_type):
        """按类型过滤"""
        ids = self.type_index.get(gene_type, [])
        return [g for g in self.genes if g["id"] in ids]
    
    def filter_by_score_range(self, min_score, max_score):
        """按评分范围过滤"""
        return [g for g in self.genes if min_score <= g.get("apex_score", 0) <= max_score]
    
    def get_statistics(self):
        """获取基因库统计"""
        harness_passed = sum(1 for g in self.genes if g.get("harness_audit", {}).get("passed", False))
        scores = [g.get("apex_score", 0) for g in self.genes]
        
        return {
            "total": len(self.genes),
            "harness_passed": harness_passed,
            "avg_score": round(sum(scores) / max(len(scores), 1), 4),
            "max_score": max(scores) if scores else 0,
            "min_score": min(scores) if scores else 0,
            "categories": list(self.type_index.keys())
        }

if __name__ == "__main__":
    gs = GeneStorage(r"C:\Users\Administrator\ApexSpiral\evolution-core\gene_bank.json")
    count = gs.load()
    print(f"Loaded {count} genes")
    
    stats = gs.get_statistics()
    print(json.dumps(stats, indent=2))
    
    results = gs.search("quantize attention", top_k=5)
    print(f"Search results: {[g['id'] for g in results]}")
