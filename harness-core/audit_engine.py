#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
Life-Harness Audit Core
四层审计引擎：Security / Robustness / Architecture / Performance
"""

import re, ast, pathlib

def audit_security(code: str) -> dict:
    """Security: 检测危险代码模式"""
    danger = [
        (r'eval\s*\(', 'eval() 执行任意代码'),
        (r'exec\s*\(', 'exec() 执行任意代码'),
        (r'subprocess\s*\.(run|call|Popen)\s*\([^,]*shell\s*=\s*True', 'shell=True 命令注入风险'),
        (r'os\.system\s*\(', 'os.system 不安全'),
        (r'sqlalchemy\s*text\s*\([^)]*\+', 'SQL 拼接注入'),
        (r'\.format\s*\([^)]*%s', '格式化字符串注入'),
        (r'requests\.get\s*\([^)]*url\s*=', 'URL 注入风险'),
        (r'pickle\.load', 'pickle 反序列化风险'),
        (r'yaml\.load\s*\([^)]*Loader\s*=\s*yaml\.FullLoader', 'yaml 不安全加载'),
    ]
    found = []
    for pat, desc in danger:
        if re.search(pat, code):
            found.append(desc)
    score = max(0, 100 - len(found) * 25)
    return {"score": score, "passed": score >= 50, "issues": found}

def audit_robustness(code: str) -> dict:
    """Robustness: 检测异常处理完整性"""
    has_try = bool(re.search(r'\btry\s*:', code))
    has_except = bool(re.search(r'\bexcept\s*:', code))
    has_finally = bool(re.search(r'\bfinally\s*:', code))
    has_logging = bool(re.search(r'logging\.(error|warning|info|debug)', code))
    has_raise = bool(re.search(r'\braise\s+', code))
    
    checks = [has_try, has_except, has_finally, has_logging, has_raise]
    score = int(sum(checks) / len(checks) * 100)
    return {
        "score": score,
        "passed": score >= 40,
        "details": {
            "has_try": has_try, "has_except": has_except,
            "has_finally": has_finally, "has_logging": has_logging,
            "has_raise": has_raise
        }
    }

def audit_architecture(code: str) -> dict:
    """Architecture: 检测设计模式与结构质量"""
    patterns = {
        "class_pattern": r'class\s+\w+\s*[\(\:]',
        "function_def": r'def\s+\w+\s*\(',
        "async_func": r'async\s+def\s+\w+\s*\(',
        "dataclass": r'@dataclass',
        "property_decorator": r'@property',
        "context_manager": r'with\s+.*\s+as\s+',
    }
    found = {k: bool(re.search(pat, code)) for k, pat in patterns.items()}
    score = int(sum(found.values()) / len(found) * 100)
    return {"score": score, "passed": score >= 30, "patterns": found}

def audit_performance(code: str) -> dict:
    """Performance: 检测异步/并行/缓存优化"""
    patterns = {
        "async_def": r'async\s+def\s+\w+\s*\(',
        "await_keyword": r'\bawait\s+',
        "threading": r'import\s+threading|from\s+threading\s+import',
        "asyncio_gather": r'asyncio\.gather',
        "multiprocessing": r'import\s+multiprocessing|from\s+multiprocessing\s+import',
        "lru_cache": r'@lru_cache|@functools\.lru_cache',
        "list_comp": r'\[.+\s+for\s+.+\s+in\s+.+\]',
        "generator": r'\([^.]+for\s+.+in\s+.+\)',
    }
    found = {k: bool(re.search(pat, code)) for k, pat in patterns.items()}
    score = int(sum(found.values()) / len(found) * 100)
    return {"score": score, "passed": score >= 25, "patterns": found}

def full_audit(code: str) -> dict:
    """执行完整四层审计，返回结构化报告"""
    security = audit_security(code)
    robustness = audit_robustness(code)
    architecture = audit_architecture(code)
    performance = audit_performance(code)
    
    layers = {
        "security": security,
        "robustness": robustness,
        "architecture": architecture,
        "performance": performance
    }
    
    avg = sum(v["score"] for v in layers.values()) / 4
    
    # 生成 Apex Score (简化版)
    apex_score = round(
        (security["score"] * 0.25 +
         robustness["score"] * 0.25 +
         architecture["score"] * 0.25 +
         performance["score"] * 0.25) / 100, 4
    )
    
    return {
        "layers": layers,
        "overall_score": round(avg, 2),
        "apex_score": apex_score,
        "passed": all(v["passed"] for v in layers.values()),
        "timestamp": str(pathlib.Path(__file__).stat().st_mtime)
    }

def audit_file(path: str) -> dict:
    """对单个文件执行完整审计"""
    p = pathlib.Path(path)
    if not p.exists():
        return {"error": "file not found"}
    code = p.read_text(encoding="utf-8", errors="ignore")
    return full_audit(code)

def audit_gene(gene_text: str) -> dict:
    """对单条基因执行审计，返回带评分的结构"""
    result = full_audit(gene_text)
    result["gene_length"] = len(gene_text)
    return result

if __name__ == "__main__":
    import sys
    if len(sys.argv) > 1:
        result = audit_file(sys.argv[1])
        import json
        print(json.dumps(result, ensure_ascii=False, indent=2))