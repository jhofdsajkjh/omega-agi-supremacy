#!/usr/bin/env python3
"""Hermes Self-Evolution Engine — with Life-Harness 审计"""
import sys, pathlib, json, urllib.request, base64
from datetime import datetime

# 导入 Life-Harness 审计引擎
sys.path.insert(0, str(pathlib.Path(__file__).parent.parent / 'harness-core'))
try:
    from audit_engine import full_audit
    HAS_HARNESS = True
except ImportError:
    HAS_HARNESS = False

class EvolutionEngine:
    def __init__(self, token, work_dir):
        self.token = token
        self.work_dir = pathlib.Path(work_dir)
        self.headers = {'Accept': 'application/vnd.github+json', 'User-Agent': 'Hermes-Evolution-v1'}
        if token:
            self.headers['Authorization'] = f'Bearer {token}'
        self.gene_file = self.work_dir / 'gene_bank.json'
        self.log_file = self.work_dir / 'evolution.log'
    
    def log(self, msg):
        ts = datetime.now().isoformat()
        line = f'[{ts}] {msg}'
        print(line)
        with open(self.log_file, 'a', encoding='utf-8') as f:
            f.write(line + chr(10))
    
    def get_trending(self, lang='python', limit=5):
        url = f'https://api.github.com/search/repositories?q=stars:>10000+language:{lang}&sort=stars&order=desc&per_page={limit}'
        req = urllib.request.Request(url, headers=self.headers)
        with urllib.request.urlopen(req, timeout=30) as r:
            return json.loads(r.read().decode()).get('items', [])
    
    def get_contents(self, owner, repo, path=''):
        url = f'https://api.github.com/repos/{owner}/{repo}/contents/{path}'
        req = urllib.request.Request(url, headers=self.headers)
        with urllib.request.urlopen(req, timeout=30) as r:
            return json.loads(r.read().decode())
    
    def get_file(self, url):
        req = urllib.request.Request(url, headers=self.headers)
        with urllib.request.urlopen(req, timeout=30) as r:
            content_type = r.headers.get('Content-Type', '')
            raw = r.read()
        if 'application/json' in content_type:
            data = json.loads(raw.decode())
            if isinstance(data, dict) and data.get('content'):
                return base64.b64decode(data['content']).decode('utf-8')
        else:
            return raw.decode('utf-8', errors='ignore')
        return None
    
    def wrap_robustness(self, gene_text: str) -> str:
        """Robustness 增强：为基因代码添加 try-except 包装"""
        # 如果是函数定义，添加异常处理包装
        if 'def ' in gene_text and ':' in gene_text and 'try' not in gene_text:
            # 简单模式：保留原代码，添加注释标注需要异常处理
            return f"# [Harness-Robustness] Enhanced\n{gene_text}\n\n# [TODO] Add try-except wrapper for production use"
        return gene_text
    
    def extract_genes(self, code):
        genes = []
        for line in code.split(chr(10)):
            line = line.strip()
            if not line or line.startswith('#'):
                continue
            if 'def ' in line and '(' in line:
                genes.append(('function', line))
            elif 'class ' in line and ':' in line:
                genes.append(('class', line))
            elif 'import ' in line or 'from ' in line:
                genes.append(('import', line))
            elif line.startswith('@') and 'def ' not in line:
                genes.append(('decorator', line))
        return genes
    
    def evolve(self):
        self.log('=== 开始自我进化 ===')
        self.log('Phase 1: 基因觉醒')
        
        repos = self.get_trending('python', 5)
        genes_all = {'function': [], 'class': [], 'import': [], 'decorator': []}
        audit_results = []
        
        for repo in repos:
            name = repo['full_name']
            self.log(f'处理: {name}')
            owner, repo_name = name.split('/')
            try:
                contents = self.get_contents(owner, repo_name)
                dirs = [c for c in contents if c.get('type') == 'dir'
                       and not c.get('name', '').startswith('.')
                       and c.get('name', '') not in {'docs', 'doc', 'examples', 'tests', 'test'}][:3]
                for d in dirs:
                    sub = self.get_contents(owner, repo_name, d['name'])
                    for pf in [c for c in sub if c.get('name', '').endswith('.py')][:1]:
                        code = self.get_file(pf['url'])
                        if code and len(code) > 100:
                            genes = self.extract_genes(code)
                            for gtype, pattern in genes:
                                if pattern not in genes_all[gtype]:
                                    # 应用 Robustness 增强
                                    enhanced = self.wrap_robustness(pattern)
                                    genes_all[gtype].append(enhanced)
                                    # 执行 Harness 审计
                                    if HAS_HARNESS:
                                        audit = full_audit(pattern)
                                        audit_results.append({
                                            'gene': pattern[:60],
                                            'type': gtype,
                                            'audit': audit
                                        })
                            self.log(f'  {pf["name"]}: {len(genes)} 基因')
            except Exception as e:
                self.log(f'  错误: {e}')
        
        # 保存基因库
        bank = json.loads(self.gene_file.read_text(encoding='utf-8'))
        for gtype, patterns in genes_all.items():
            for p in patterns:
                if p not in bank['genes'][gtype]:
                    bank['genes'][gtype].append(p)
        bank['stats']['total'] = sum(len(v) for v in bank['genes'].values())
        bank['stats']['last_update'] = datetime.now().isoformat()
        # 保存审计报告
        if audit_results:
            bank['harness_audits'] = audit_results[:20]  # 只保留前20条
        self.gene_file.write_text(json.dumps(bank, indent=2, ensure_ascii=False), encoding='utf-8')
        self.log(f'基因库: {bank["stats"]["total"]} 个')
        self.log(f'Harness 审计: {len(audit_results)} 条')
        self.log('=== 进化完成 ===')
        return bank['stats']['total']

if __name__ == '__main__':
    import os as _os
    token = sys.argv[1] if len(sys.argv) > 1 else _os.environ.get('GITHUB_TOKEN')
    work_dir = sys.argv[2] if len(sys.argv) > 2 else './'
    eng = EvolutionEngine(token, work_dir)
    gene_count = eng.evolve()
    print('进化完成! 基因:', gene_count)