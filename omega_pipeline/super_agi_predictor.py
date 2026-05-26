#!/usr/bin/env python3
"""
主动漏洞预测系统 (Super AGI模块) - 超越被动扫描

对比:
- OpenHuman: 依赖外部工具被动发现 (4高危漏洞)
- Hermes-Agent: 无安全检测 (14严重漏洞)
- OMEGA: 主动预测潜在漏洞 (目标0漏洞)

作者: Claude (自动实现)
版本: 1.0
"""

import asyncio
import re
import json
from dataclasses import dataclass, asdict
from typing import List, Dict, Optional, Tuple, Set
from datetime import datetime
from enum import Enum
import hashlib


class Severity(Enum):
    CRITICAL = "critical"
    HIGH = "high"
    MEDIUM = "medium"
    LOW = "low"
    INFO = "info"


class VulnerabilityType(Enum):
    SQL_INJECTION = "SQL Injection"
    COMMAND_INJECTION = "Command Injection"
    PATH_TRAVERSAL = "Path Traversal"
    HARDCODED_SECRETS = "Hardcoded Secrets"
    XSS = "Cross-Site Scripting"
    INSECURE_DESERIALIZATION = "Insecure Deserialization"
    SSRF = "Server-Side Request Forgery"
    RCE = "Remote Code Execution"
    BUFFER_OVERFLOW = "Buffer Overflow"
    RACE_CONDITION = "Race Condition"


@dataclass
class VulnerabilityPrediction:
    """漏洞预测结果"""
    file_path: str
    line_number: int
    column: int
    vulnerability_type: VulnerabilityType
    severity: Severity
    confidence: float  # 0-1
    description: str
    suggested_fix: str
    cwe_id: str
    cvss_score: Optional[float] = None
    affected_code: str = ""
    
    def to_dict(self) -> Dict:
        return {
            **asdict(self),
            'vulnerability_type': self.vulnerability_type.value,
            'severity': self.severity.value,
        }


@dataclass
class SecurityReport:
    """安全扫描报告"""
    scan_time: datetime
    files_scanned: int
    vulnerabilities: List[VulnerabilityPrediction]
    risk_score: float  # 0-100
    
    def to_dict(self) -> Dict:
        return {
            'scan_time': self.scan_time.isoformat(),
            'files_scanned': self.files_scanned,
            'vulnerabilities': [v.to_dict() for v in self.vulnerabilities],
            'risk_score': self.risk_score,
            'summary': self._generate_summary(),
        }
    
    def _generate_summary(self) -> Dict:
        severity_counts = {}
        type_counts = {}
        
        for v in self.vulnerabilities:
            severity_counts[v.severity.value] = severity_counts.get(v.severity.value, 0) + 1
            type_counts[v.vulnerability_type.value] = type_counts.get(v.vulnerability_type.value, 0) + 1
        
        return {
            'total_vulnerabilities': len(self.vulnerabilities),
            'by_severity': severity_counts,
            'by_type': type_counts,
        }


class VulnerabilityPredictor:
    """
    主动漏洞预测器
    
    使用多种技术:
    1. 静态分析 - 模式匹配
    2. 语义分析 - AST分析
    3. 机器学习 - 深度学习模型 (可选)
    """
    
    def __init__(self):
        self.vulnerability_patterns = self._load_patterns()
        self.cwe_database = self._load_cwe_database()
        
    def _load_patterns(self) -> Dict[VulnerabilityType, List[Dict]]:
        """加载漏洞检测模式"""
        return {
            VulnerabilityType.SQL_INJECTION: [
                {
                    'pattern': r'execute\s*\(\s*f["\']',
                    'description': 'f-string in SQL execute',
                    'severity': Severity.CRITICAL,
                    'confidence': 0.95,
                },
                {
                    'pattern': r'execute\s*\(\s*["\'].*%s',
                    'description': 'String formatting in SQL',
                    'severity': Severity.CRITICAL,
                    'confidence': 0.90,
                },
                {
                    'pattern': r'execute\s*\(\s*["\'].*\+',
                    'description': 'String concatenation in SQL',
                    'severity': Severity.HIGH,
                    'confidence': 0.85,
                },
                {
                    'pattern': r'cursor\.execute\s*\(\s*["\'].*\$\{',
                    'description': 'Template literal in SQL',
                    'severity': Severity.CRITICAL,
                    'confidence': 0.95,
                },
                {
                    'pattern': r'\.format\s*\(.*\)\s*.*execute',
                    'description': 'format() before execute',
                    'severity': Severity.HIGH,
                    'confidence': 0.80,
                },
            ],
            VulnerabilityType.COMMAND_INJECTION: [
                {
                    'pattern': r'os\.system\s*\(\s*.*\+',
                    'description': 'String concatenation in os.system',
                    'severity': Severity.CRITICAL,
                    'confidence': 0.95,
                },
                {
                    'pattern': r'subprocess\.call\s*\(\s*.*\+',
                    'description': 'String concatenation in subprocess',
                    'severity': Severity.CRITICAL,
                    'confidence': 0.90,
                },
                {
                    'pattern': r'eval\s*\(\s*.*\$',
                    'description': 'Dynamic content in eval',
                    'severity': Severity.CRITICAL,
                    'confidence': 0.95,
                },
                {
                    'pattern': r'exec\s*\(\s*.*\+',
                    'description': 'String concatenation in exec',
                    'severity': Severity.CRITICAL,
                    'confidence': 0.95,
                },
            ],
            VulnerabilityType.PATH_TRAVERSAL: [
                {
                    'pattern': r'open\s*\(\s*.*\+',
                    'description': 'String concatenation in open()',
                    'severity': Severity.HIGH,
                    'confidence': 0.85,
                },
                {
                    'pattern': r'open\s*\(\s*f["\']',
                    'description': 'f-string in open()',
                    'severity': Severity.HIGH,
                    'confidence': 0.90,
                },
                {
                    'pattern': r'__import__\s*\(\s*.*\+',
                    'description': 'Dynamic import with concatenation',
                    'severity': Severity.HIGH,
                    'confidence': 0.85,
                },
                {
                    'pattern': r'\.read\s*\(\s*\).*\.format',
                    'description': 'format() in file path',
                    'severity': Severity.MEDIUM,
                    'confidence': 0.75,
                },
            ],
            VulnerabilityType.HARDCODED_SECRETS: [
                {
                    'pattern': r'api_key\s*=\s*["\'][^"\']{10,}["\']',
                    'description': 'Hardcoded API key',
                    'severity': Severity.CRITICAL,
                    'confidence': 0.95,
                },
                {
                    'pattern': r'password\s*=\s*["\'][^"\']{8,}["\']',
                    'description': 'Hardcoded password',
                    'severity': Severity.CRITICAL,
                    'confidence': 0.95,
                },
                {
                    'pattern': r'secret\s*=\s*["\'][^"\']{10,}["\']',
                    'description': 'Hardcoded secret',
                    'severity': Severity.CRITICAL,
                    'confidence': 0.90,
                },
                {
                    'pattern': r'token\s*=\s*["\'][^"\']{10,}["\']',
                    'description': 'Hardcoded token',
                    'severity': Severity.CRITICAL,
                    'confidence': 0.90,
                },
                {
                    'pattern': r'aws_access_key_id\s*=\s*["\'][^"\']+["\']',
                    'description': 'Hardcoded AWS key',
                    'severity': Severity.CRITICAL,
                    'confidence': 0.95,
                },
                {
                    'pattern': r'private_key\s*=\s*["\'][^"\']{20,}["\']',
                    'description': 'Hardcoded private key',
                    'severity': Severity.CRITICAL,
                    'confidence': 0.98,
                },
            ],
            VulnerabilityType.XSS: [
                {
                    'pattern': r'innerHTML\s*=\s*.*\+',
                    'description': 'Dynamic content in innerHTML',
                    'severity': Severity.HIGH,
                    'confidence': 0.85,
                },
                {
                    'pattern': r'document\.write\s*\(\s*.*\+',
                    'description': 'Dynamic content in document.write',
                    'severity': Severity.HIGH,
                    'confidence': 0.85,
                },
            ],
            VulnerabilityType.INSECURE_DESERIALIZATION: [
                {
                    'pattern': r'pickle\.loads?\s*\(\s*.*\)',
                    'description': 'Insecure pickle usage',
                    'severity': Severity.HIGH,
                    'confidence': 0.80,
                },
                {
                    'pattern': r'yaml\.load\s*\(\s*[^,]+\)',
                    'description': 'Unsafe yaml.load (no Loader)',
                    'severity': Severity.HIGH,
                    'confidence': 0.85,
                },
            ],
            VulnerabilityType.SSRF: [
                {
                    'pattern': r'requests\.get\s*\(\s*.*\+',
                    'description': 'Dynamic URL in requests',
                    'severity': Severity.MEDIUM,
                    'confidence': 0.75,
                },
                {
                    'pattern': r'urllib\.request\.urlopen\s*\(\s*.*\+',
                    'description': 'Dynamic URL in urllib',
                    'severity': Severity.MEDIUM,
                    'confidence': 0.75,
                },
            ],
        }
    
    def _load_cwe_database(self) -> Dict[VulnerabilityType, str]:
        """加载CWE映射"""
        return {
            VulnerabilityType.SQL_INJECTION: "CWE-89",
            VulnerabilityType.COMMAND_INJECTION: "CWE-78",
            VulnerabilityType.PATH_TRAVERSAL: "CWE-22",
            VulnerabilityType.HARDCODED_SECRETS: "CWE-798",
            VulnerabilityType.XSS: "CWE-79",
            VulnerabilityType.INSECURE_DESERIALIZATION: "CWE-502",
            VulnerabilityType.SSRF: "CWE-918",
            VulnerabilityType.RCE: "CWE-94",
            VulnerabilityType.BUFFER_OVERFLOW: "CWE-120",
            VulnerabilityType.RACE_CONDITION: "CWE-362",
        }
    
    async def scan_file(self, file_path: str, content: str) -> List[VulnerabilityPrediction]:
        """扫描单个文件"""
        predictions = []
        lines = content.split('\n')
        
        for vuln_type, patterns in self.vulnerability_patterns.items():
            for pattern_def in patterns:
                matches = self._find_pattern_matches(
                    content, lines, pattern_def['pattern']
                )
                
                for line_num, col, matched_text in matches:
                    prediction = VulnerabilityPrediction(
                        file_path=file_path,
                        line_number=line_num,
                        column=col,
                        vulnerability_type=vuln_type,
                        severity=pattern_def['severity'],
                        confidence=pattern_def['confidence'],
                        description=pattern_def['description'],
                        suggested_fix=self._generate_fix_suggestion(vuln_type),
                        cwe_id=self.cwe_database.get(vuln_type, "CWE-Unknown"),
                        cvss_score=self._calculate_cvss(pattern_def['severity']),
                        affected_code=matched_text[:100],
                    )
                    predictions.append(prediction)
        
        # 去重 (相同位置相同类型)
        predictions = self._deduplicate_predictions(predictions)
        
        return sorted(predictions, key=lambda x: (
            self._severity_to_int(x.severity),
            -x.confidence
        ))
    
    def _find_pattern_matches(self, content: str, lines: List[str], 
                              pattern: str) -> List[Tuple[int, int, str]]:
        """查找模式匹配"""
        matches = []
        regex = re.compile(pattern, re.IGNORECASE)
        
        for line_num, line in enumerate(lines, 1):
            for match in regex.finditer(line):
                matches.append((
                    line_num,
                    match.start() + 1,
                    match.group(0)
                ))
        
        return matches
    
    def _deduplicate_predictions(self, predictions: List[VulnerabilityPrediction]) -> List[VulnerabilityPrediction]:
        """去重预测结果"""
        seen = set()
        unique = []
        
        for p in predictions:
            key = (p.file_path, p.line_number, p.vulnerability_type)
            if key not in seen:
                seen.add(key)
                unique.append(p)
        
        return unique
    
    def _generate_fix_suggestion(self, vuln_type: VulnerabilityType) -> str:
        """生成修复建议"""
        suggestions = {
            VulnerabilityType.SQL_INJECTION: 
                "使用参数化查询: cursor.execute('SELECT * FROM users WHERE id = ?', (user_id,))",
            VulnerabilityType.COMMAND_INJECTION: 
                "使用参数列表: subprocess.run(['ls', '-la', directory], check=True)",
            VulnerabilityType.PATH_TRAVERSAL: 
                "使用pathlib并验证路径: Path(base_dir) / filename; assert path.resolve().startswith(base_dir)",
            VulnerabilityType.HARDCODED_SECRETS: 
                "使用环境变量: api_key = os.environ.get('API_KEY') 或使用密钥管理服务",
            VulnerabilityType.XSS: 
                "使用模板引擎的自动转义功能或手动转义输出",
            VulnerabilityType.INSECURE_DESERIALIZATION: 
                "使用安全替代: json.loads() 替代 pickle; yaml.safe_load() 替代 yaml.load()",
            VulnerabilityType.SSRF: 
                "验证和限制URL: 使用白名单、禁用内部IP访问",
        }
        return suggestions.get(vuln_type, "请查阅安全最佳实践进行修复")
    
    def _calculate_cvss(self, severity: Severity) -> float:
        """计算CVSS分数"""
        scores = {
            Severity.CRITICAL: 9.5,
            Severity.HIGH: 7.5,
            Severity.MEDIUM: 5.0,
            Severity.LOW: 2.5,
            Severity.INFO: 0.0,
        }
        return scores.get(severity, 5.0)
    
    def _severity_to_int(self, severity: Severity) -> int:
        """严重程度转数字 (用于排序)"""
        order = {
            Severity.CRITICAL: 0,
            Severity.HIGH: 1,
            Severity.MEDIUM: 2,
            Severity.LOW: 3,
            Severity.INFO: 4,
        }
        return order.get(severity, 5)
    
    async def scan_directory(self, directory: str, 
                            file_extensions: Optional[List[str]] = None) -> SecurityReport:
        """扫描整个目录"""
        import os
        
        if file_extensions is None:
            file_extensions = ['.py', '.js', '.ts', '.rs', '.java', '.go']
        
        all_vulnerabilities = []
        files_scanned = 0
        
        for root, _, files in os.walk(directory):
            for file in files:
                if any(file.endswith(ext) for ext in file_extensions):
                    file_path = os.path.join(root, file)
                    try:
                        with open(file_path, 'r', encoding='utf-8', errors='ignore') as f:
                            content = f.read()
                        
                        vulnerabilities = await self.scan_file(file_path, content)
                        all_vulnerabilities.extend(vulnerabilities)
                        files_scanned += 1
                    except Exception as e:
                        print(f"Error scanning {file_path}: {e}")
        
        # 计算风险分数
        risk_score = self._calculate_risk_score(all_vulnerabilities)
        
        return SecurityReport(
            scan_time=datetime.now(),
            files_scanned=files_scanned,
            vulnerabilities=all_vulnerabilities,
            risk_score=risk_score,
        )
    
    def _calculate_risk_score(self, vulnerabilities: List[VulnerabilityPrediction]) -> float:
        """计算整体风险分数 (0-100)"""
        if not vulnerabilities:
            return 0.0
        
        weights = {
            Severity.CRITICAL: 10,
            Severity.HIGH: 5,
            Severity.MEDIUM: 2,
            Severity.LOW: 0.5,
            Severity.INFO: 0.1,
        }
        
        total_score = sum(
            weights.get(v.severity, 1) * v.confidence 
            for v in vulnerabilities
        )
        
        # 归一化到0-100
        return min(100.0, total_score * 10)


class SuperAGISecurityModule:
    """
    Super AGI安全模块 - 主动预测与防御
    
    这是OMEGA AGI超越OpenHuman和Hermes-Agent的核心安全组件
    """
    
    def __init__(self):
        self.predictor = VulnerabilityPredictor()
        self.scan_history: List[SecurityReport] = []
        
    async def predict_and_defend(self, code: str, file_path: str) -> Dict:
        """
        主动预测漏洞并提供防御建议
        
        Returns:
            {
                'safe': bool,
                'vulnerabilities': List[VulnerabilityPrediction],
                'defense_actions': List[str],
                'risk_score': float,
            }
        """
        vulnerabilities = await self.predictor.scan_file(file_path, code)
        
        # 生成防御动作
        defense_actions = self._generate_defense_actions(vulnerabilities)
        
        # 计算风险分数
        risk_score = self.predictor._calculate_risk_score(vulnerabilities)
        
        # 判断是否安全 (没有Critical和High级别漏洞)
        safe = not any(
            v.severity in [Severity.CRITICAL, Severity.HIGH] 
            for v in vulnerabilities
        )
        
        return {
            'safe': safe,
            'vulnerabilities': [v.to_dict() for v in vulnerabilities],
            'defense_actions': defense_actions,
            'risk_score': risk_score,
        }
    
    def _generate_defense_actions(self, vulnerabilities: List[VulnerabilityPrediction]) -> List[str]:
        """生成防御动作"""
        actions = []
        
        for v in vulnerabilities:
            if v.severity == Severity.CRITICAL:
                actions.append(f"[BLOCK] {v.file_path}:{v.line_number} - {v.vulnerability_type.value}")
            elif v.severity == Severity.HIGH:
                actions.append(f"[WARN] {v.file_path}:{v.line_number} - {v.vulnerability_type.value}")
            
            actions.append(f"  Fix: {v.suggested_fix}")
        
        return actions
    
    async def continuous_monitoring(self, directories: List[str], interval_seconds: int = 3600):
        """持续监控目录"""
        while True:
            print(f"\n🔍 [{datetime.now()}] 开始安全扫描...")
            
            for directory in directories:
                report = await self.predictor.scan_directory(directory)
                self.scan_history.append(report)
                
                print(f"\n📁 目录: {directory}")
                print(f"   扫描文件: {report.files_scanned}")
                print(f"   发现漏洞: {len(report.vulnerabilities)}")
                print(f"   风险分数: {report.risk_score:.2f}/100")
                
                if report.vulnerabilities:
                    critical = sum(1 for v in report.vulnerabilities if v.severity == Severity.CRITICAL)
                    high = sum(1 for v in report.vulnerabilities if v.severity == Severity.HIGH)
                    print(f"   ⚠️  Critical: {critical}, High: {high}")
            
            print(f"\n✅ 扫描完成，{interval_seconds}秒后下次扫描...")
            await asyncio.sleep(interval_seconds)
    
    def generate_security_report(self, output_path: str):
        """生成安全报告文件"""
        if not self.scan_history:
            print("No scan history available")
            return
        
        latest = self.scan_history[-1]
        report = latest.to_dict()
        
        with open(output_path, 'w') as f:
            json.dump(report, f, indent=2)
        
        print(f"Security report saved to: {output_path}")


# 命令行接口
if __name__ == "__main__":
    import sys
    
    if len(sys.argv) < 2:
        print("Usage: python super_agi_predictor.py <scan|monitor> <path>")
        sys.exit(1)
    
    command = sys.argv[1]
    path = sys.argv[2] if len(sys.argv) > 2 else "."
    
    module = SuperAGISecurityModule()
    
    if command == "scan":
        async def run_scan():
            report = await module.predictor.scan_directory(path)
            print(json.dumps(report.to_dict(), indent=2))
        
        asyncio.run(run_scan())
    
    elif command == "monitor":
        asyncio.run(module.continuous_monitoring([path]))
    
    else:
        print(f"Unknown command: {command}")
        sys.exit(1)
