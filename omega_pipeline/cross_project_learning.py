"""
Cross-Project Learning System for OMEGA AGI Supremacy
从多个项目中学习模式，提取可复用的解决方案
Author: OMEGA AGI Version: 0.1.0
"""
from __future__ import annotations
import os, re, hashlib, sqlite3, json
from dataclasses import dataclass, field
from typing import Dict, List, Optional, Tuple
from datetime import datetime
from enum import Enum

class PatternType(Enum):
    CODE = "code"
    TEST = "test"
    ERROR = "error"
    ARCHITECTURE = "architecture"

@dataclass
class LearningConfig:
    embedding_model: str = "tfidf"
    similarity_threshold: float = 0.75
    max_patterns_per_project: int = 1000
    min_success_rate: float = 0.6
    enable_github_sync: bool = True
    github_token: Optional[str] = None
    db_path: str = ".cross_project_learning.db"

@dataclass
class Pattern:
    id: str; type: PatternType; project: str; file_path: str
    code_snippet: str; description: str; usage_count: int = 0
    success_rate: float = 0.0; embeddings: List[float] = field(default_factory=list)
    created_at: str = ""
    def __post_init__(self):
        if not self.created_at: self.created_at = datetime.now().isoformat()

@dataclass
class PatternDB:
    patterns: Dict[str, Pattern] = field(default_factory=dict)
    index: Dict[PatternType, List[str]] = field(default_factory=dict)
    def add_pattern(self, pattern: Pattern) -> None:
        self.patterns[pattern.id] = pattern
        if pattern.type not in self.index: self.index[pattern.type] = []
        if pattern.id not in self.index[pattern.type]: self.index[pattern.type].append(pattern.id)
    def get_by_type(self, pt: PatternType) -> List[Pattern]:
        ids = self.index.get(pt, []); return [self.patterns[pid] for pid in ids if pid in self.patterns]
    def get_similar(self, emb: List[float], threshold: float = 0.75) -> List[Tuple[Pattern, float]]:
        results = []
        for p in self.patterns.values():
            if p.embeddings:
                s = cosine_similarity(emb, p.embeddings)
                if s >= threshold: results.append((p, s))
        results.sort(key=lambda x: x[1], reverse=True); return results

@dataclass
class ProjectIndex:
    project_id: str; project_path: str; file_count: int = 0
    total_lines: int = 0; languages: List[str] = field(default_factory=list)
    patterns_found: int = 0; indexed_at: str = ""
    def __post_init__(self):
        if not self.indexed_at: self.indexed_at = datetime.now().isoformat()

@dataclass
class Learning:
    id: str; project: str; pattern_count: int; insights_generated: int; timestamp: str = ""
    def __post_init__(self):
        if not self.timestamp: self.timestamp = datetime.now().isoformat()

@dataclass
class ProblemContext:
    description: str; language: str; code_snippet: str; error_message: Optional[str] = None

@dataclass
class Solution:
    pattern: Pattern; confidence: float; applied_code: str

@dataclass
class Insight:
    pattern_id: str; insight_text: str; applicability: str

@dataclass
class LearningResult:
    projects_processed: int; patterns_extracted: int; insights_generated: int; errors: List[str] = field(default_factory=list)

def cosine_similarity(v1: List[float], v2: List[float]) -> float:
    if len(v1) != len(v2) or len(v1) == 0: return 0.0
    dot = sum(a*b for a,b in zip(v1,v2))
    mag1 = sum(a*a for a in v1)**0.5; mag2 = sum(b*b for b in v2)**0.5
    return dot/(mag1*mag2) if mag1 and mag2 else 0.0

def simple_hash(text: str, dims: int = 128) -> List[float]:
    emb = [0.0]*dims
    for i, c in enumerate(text): emb[i%dims] += ord(c)*(i+1)
    mag = sum(e*e for e in emb)**0.5
    return [e/mag for e in emb] if mag > 0 else emb

def extract_signatures(code: str, lang: str) -> List[str]:
    if lang == "rust":
        return re.findall(r'fn\s+([a-zA-Z_][a-zA-Z0-9_]*)\s*\([^)]*\)', code)
    elif lang == "python":
        return re.findall(r'def\s+([a-zA-Z_][a-zA-Z0-9_]*)\s*\([^)]*\):', code)
    return []

def detect_lang(fp: str) -> Optional[str]:
    ext = os.path.splitext(fp)[1].lower()
    return {".rs": "rust", ".py": "python"}.get(ext)

class LearningDatabase:
    def __init__(self, db_path: str):
        self.db_path = db_path
        self.conn = sqlite3.connect(db_path)
        self.conn.execute("""
            CREATE TABLE IF NOT EXISTS patterns (
                id TEXT PRIMARY KEY, type TEXT NOT NULL, project TEXT NOT NULL,
                file_path TEXT NOT NULL, code_snippet TEXT NOT NULL, description TEXT,
                usage_count INTEGER DEFAULT 0, success_rate REAL DEFAULT 0.0,
                embeddings TEXT, created_at TEXT)""")
        self.conn.execute("""
            CREATE TABLE IF NOT EXISTS learnings (
                id TEXT PRIMARY KEY, project TEXT NOT NULL, pattern_count INTEGER,
                insights_generated INTEGER, timestamp TEXT)""")
        self.conn.commit()
    def save_pattern(self, p: Pattern) -> None:
        self.conn.execute("""
            INSERT OR REPLACE INTO patterns VALUES (?,?,?,?,?,?,?,?,?,?)""",
            (p.id, p.type.value, p.project, p.file_path, p.code_snippet, p.description,
             p.usage_count, p.success_rate, json.dumps(p.embeddings), p.created_at))
        self.conn.commit()
    def save_learning(self, l: Learning) -> None:
        self.conn.execute("INSERT INTO learnings VALUES (?,?,?,?,?)",
            (l.id, l.project, l.pattern_count, l.insights_generated, l.timestamp))
        self.conn.commit()
    def close(self) -> None: self.conn.close()

class CrossProjectLearner:
    def __init__(self, config: Optional[LearningConfig] = None):
        self.config = config or LearningConfig()
        self.db = LearningDatabase(self.config.db_path)
        self.project_index: Dict[str, ProjectIndex] = {}
        self.patterns = PatternDB()
        self.learnings: List[Learning] = []
    def index_project(self, project_path: str) -> ProjectIndex:
        pid = hashlib.md5(project_path.encode()).hexdigest()[:12]
        fc, tl, langs = 0, 0, set()
        for root, dirs, files in os.walk(project_path):
            dirs[:] = [d for d in dirs if not d.startswith(".") and d not in ["target","__pycache__","node_modules","venv"]]
            for file in files:
                fp = os.path.join(root, file); lang = detect_lang(fp)
                if lang in ["rust","python"]:
                    fc += 1; langs.add(lang)
                    try:
                        with open(fp, "r", encoding="utf-8", errors="ignore") as f: tl += len(f.readlines())
                    except: pass
        idx = ProjectIndex(project_id=pid, project_path=project_path, file_count=fc, total_lines=tl, languages=list(langs))
        self.project_index[pid] = idx; return idx
    def extract_patterns(self, project_id: str) -> List[Pattern]:
        proj = self.project_index.get(project_id)
        if not proj: return []
        patterns = []
        for root, dirs, files in os.walk(proj.project_path):
            dirs[:] = [d for d in dirs if not d.startswith(".") and d not in ["target","__pycache__","venv"]]
            for file in files:
                fp = os.path.join(root, file); lang = detect_lang(fp)
                if lang not in ["rust","python"]: continue
                try:
                    with open(fp, "r", encoding="utf-8", errors="ignore") as f: content = f.read()
                    for sig in extract_signatures(content, lang):
                        p = Pattern(id=hashlib.md5(f"{project_id}:{fp}:{sig}".encode()).hexdigest()[:16],
                            type=PatternType.CODE, project=proj.project_path, file_path=fp,
                            code_snippet=sig, description=f"{lang} function: {sig}", embeddings=simple_hash(sig))
                        patterns.append(p)
                except: pass
        return patterns[:self.config.max_patterns_per_project]
    def learn_from_projects(self, project_paths: List[str]) -> LearningResult:
        result = LearningResult(projects_processed=0, patterns_extracted=0, insights_generated=0)
        for pp in project_paths:
            try:
                idx = self.index_project(pp)
                pats = self.extract_patterns(idx.project_id)
                for p in pats:
                    self.patterns.add_pattern(p); self.db.save_pattern(p)
                lr = Learning(id=hashlib.md5(str(datetime.now()).encode()).hexdigest()[:16],
                    project=pp, pattern_count=len(pats), insights_generated=0)
                self.learnings.append(lr); self.db.save_learning(lr)
                for p in pats[:5]:
                    ins = self.generate_insight(p)
                    if ins: result.insights_generated += 1
                result.projects_processed += 1; result.patterns_extracted += len(pats)
            except Exception as e: result.errors.append(f"{pp}: {e}")
        return result
    def find_similar_solution(self, problem: ProblemContext) -> Optional[Solution]:
        emb = simple_hash(f"{problem.description} {problem.code_snippet}")
        similar = self.patterns.get_similar(emb, self.config.similarity_threshold)
        if not similar: return None
        best, sim = similar[0]; return Solution(pattern=best, confidence=sim, applied_code=best.code_snippet)
    def generate_insight(self, p: Pattern) -> Optional[Insight]:
        if p.type == PatternType.CODE: text = f"Found {p.description}"; app = "high"
        elif p.type == PatternType.ARCHITECTURE: text = f"Using {p.code_snippet}"; app = "medium"
        elif p.type == PatternType.TEST: text = f"Test pattern: {p.description}"; app = "high"
        elif p.type == PatternType.ERROR: text = f"Error pattern: {p.description}"; app = "medium"
        else: return None
        return Insight(pattern_id=p.id, insight_text=text, applicability=app)
    def get_stats(self) -> Dict:
        return {"projects": len(self.project_index), "patterns": len(self.patterns.patterns),
                "learnings": len(self.learnings),
                "by_type": {pt.value: len(self.patterns.index.get(pt,[])) for pt in PatternType}}
    def __del__(self): 
        if hasattr(self, "db"): self.db.close()

if __name__ == "__main__":
    config = LearningConfig(embedding_model="hash", similarity_threshold=0.75)
    learner = CrossProjectLearner(config)
    pp = "/root/omega-agi-supremacy/omega_pipeline"
    if os.path.exists(pp):
        idx = learner.index_project(pp); print(f"Indexed {idx.file_count} files, {idx.total_lines} lines")
        pats = learner.extract_patterns(idx.project_id); print(f"Extracted {len(pats)} patterns")
    stats = learner.get_stats(); print(f"Stats: {stats}")
