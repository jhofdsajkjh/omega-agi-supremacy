#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
Apex AGI 智能体核心引擎
感知 → 认知 → 决策 → 执行 → 反思 → 进化
"""

import json, time, datetime, traceback
from collections import defaultdict

class ApexAGI:
    def __init__(self, gene_bank_path, evo_repo, github_token):
        self.gene_bank_path = gene_bank_path
        self.evo_repo = evo_repo
        self.github_token = github_token
        self.memory = {"short_term": [], "long_term": [], "experience": []}
        self.plan = []
        self.loop_count = 0
        
    def perceive(self, input_text):
        """感知层: 解析输入、上下文、意图"""
        return {
            "raw": input_text,
            "intent": self._classify_intent(input_text),
            "entities": self._extract_entities(input_text),
            "context": self._build_context()
        }
    
    def _classify_intent(self, text):
        intents = ["code", "research", "planning", "debug", "create", "review", "evolve"]
        text_lower = text.lower()
        scores = {i: sum(1 for w in i.split("_") if w in text_lower) for i in intents}
        return max(scores, key=scores.get) if max(scores.values()) > 0 else "general"
    
    def _extract_entities(self, text):
        import re
        paths = re.findall(r'[A-Za-z]:\\[^\s]+|/[^\s]+', text)
        code_snippets = re.findall(r'`[^`]+`', text)
        return {"paths": paths, "snippets": code_snippets, "words": text.split()[:20]}
    
    def _build_context(self):
        recent = self.memory["short_term"][-5:]
        return {"recent_tasks": [r["task"] for r in recent], "loop": self.loop_count}
    
    def cognize(self, perception):
        """认知层: 基于基因库+记忆做决策"""
        # 加载基因库
        with open(self.gene_bank_path, 'r', encoding='utf-8') as f:
            genes = json.load(f)
        
        # 基于 intent 调用相关基因
        relevant_genes = [g for g in genes if perception["intent"] in g.get("source", "").lower() or perception["intent"] in g.get("name", "")]
        
        return {
            "intent": perception["intent"],
            "relevant_genes": relevant_genes[:5],
            "strategy": self._select_strategy(perception),
            "confidence": 0.85
        }
    
    def _select_strategy(self, perception):
        strategies = {
            "code": "Execute code via terminal or execute_code tool",
            "research": "Search + retrieve from knowledge pool",
            "planning": "Decompose to sub-tasks, delegate or execute",
            "debug": "Reproduce error, analyze stack trace, fix",
            "create": "Write files, validate, test",
            "review": "Analyze code structure, lint, suggest improvements",
            "evolve": "Extract code genes, update gene bank"
        }
        return strategies.get(perception["intent"], "General reasoning")
    
    def act(self, cognition):
        """执行层: 根据策略执行任务"""
        results = []
        for gene in cognition.get("relevant_genes", []):
            results.append({
                "gene_id": gene["id"],
                "name": gene["name"],
                "apex_score": gene.get("apex_score", 0),
                "harness": gene.get("harness_audit", {})
            })
        return {"actions": results, "strategy": cognition["strategy"]}
    
    def reflect(self, action_result):
        """反思层: Reflexion 框架 - 自我评估"""
        confidence = action_result.get("confidence", 0)
        harness = action_result.get("harness_audit", {}).get("layers", {})
        
        scores = {
            "security": harness.get("security", {}).get("score", 0),
            "robustness": harness.get("robustness", {}).get("score", 0),
            "architecture": harness.get("architecture", {}).get("score", 0),
            "performance": harness.get("performance", {}).get("score", 0)
        }
        
        avg_score = sum(scores.values()) / len(scores)
        reflection = {
            "loop": self.loop_count,
            "confidence": confidence,
            "layer_scores": scores,
            "avg_score": round(avg_score, 3),
            "improvement": "need_robustness" if scores["robustness"] < 0.5 else "optimal"
        }
        
        # 更新记忆
        self.memory["experience"].append(reflection)
        self.loop_count += 1
        return reflection
    
    def evolve(self, reflection):
        """进化层: 基于反思结果更新基因库"""
        if reflection["avg_score"] < 0.5:
            return {"action": "skip", "reason": "Score below threshold"}
        
        return {
            "action": "candidate",
            "loop": reflection["loop"],
            "score": reflection["avg_score"],
            "status": "evolved"
        }
    
    def run_loop(self, input_text):
        """完整 AGI 循环"""
        p = self.perceive(input_text)
        c = self.cognize(p)
        a = self.act(c)
        r = self.reflect(a)
        e = self.evolve(r)
        
        return {
            "perception": p,
            "cognition": c,
            "action": a,
            "reflection": r,
            "evolution": e,
            "timestamp": datetime.datetime.now().isoformat()
        }

# 测试运行
if __name__ == "__main__":
    agi = ApexAGI(
        gene_bank_path=r"C:\Users\Administrator\ApexSpiral\evolution-core\gene_bank.json",
        evo_repo=("jhofdsajkjh", "ai-groupchat-evolution"),
        github_token=open(r"C:\Users\Administrator\.hermes\github_token.txt").read().strip()
    )
    result = agi.run_loop("Analyze the performance of Flash Attention implementation")
    print(json.dumps(result, ensure_ascii=False, indent=2))
