import json
import re
import sys
from datetime import datetime
from pathlib import Path

class EvolutionEngine:
    def __init__(self, token=None, work_dir='./'):
        self.work_dir = Path(work_dir)
        self.gene_file = self.work_dir / 'gene_bank.json'
        self.log_file = self.work_dir / 'evolver.log'
        
    def log(self, msg):
        ts = datetime.now().isoformat()
        print(f'[{ts}] {msg}')
        with open(self.log_file, 'a', encoding='utf-8') as f:
            f.write(f'[{ts}] {msg}\n')

    def evolve(self):
        self.log('=== 开始自我进化 ===')
        self.log('Phase 1: 基因觉醒')
        
        # 模拟基因提取逻辑
        genes_all = {
            'function': [],
            'class': [],
            'import': [],
            'formula': []
        }
        
        # 修复逻辑: 处理 gene_bank.json 的旧格式并迁移
        if not self.gene_file.exists():
            bank = {
                'genes': {k: [] for k in genes_all.keys()},
                'stats': {'total': 0, 'last_update': None},
                'harness_audits': []
            }
        else:
            try:
                raw_data = json.loads(self.gene_file.read_text(encoding='utf-8'))
                if isinstance(raw_data, list):
                    self.log('检测到旧版列表格式，正在迁移到结构化字典...')
                    bank = {
                        'genes': {k: [] for k in genes_all.keys()},
                        'stats': {'total': 0, 'last_update': None},
                        'harness_audits': []
                    }
                    for item in raw_data:
                        gtype = item.get('type')
                        content = item.get('content')
                        if gtype in bank['genes'] and content:
                            if content not in bank['genes'][gtype]:
                                bank['genes'][gtype].append(content)
                else:
                    bank = raw_data
            except Exception as e:
                self.log(f'读取基因库失败: {e}')
                bank = {
                    'genes': {k: [] for k in genes_all.keys()},
                    'stats': {'total': 0, 'last_update': None},
                    'harness_audits': []
                }

        # 模拟处理过程...
        self.log('处理: TheAlgorithms/Python')
        
        # 合并新基因
        for gtype, patterns in genes_all.items():
            if gtype not in bank['genes']:
                bank['genes'][gtype] = []
            for p in patterns:
                if p not in bank['genes'][gtype]:
                    bank['genes'][gtype].append(p)
                    
        bank['stats']['total'] = sum(len(v) for v in bank['genes'].values())
        bank['stats']['last_update'] = datetime.now().isoformat()
        
        self.gene_file.write_text(json.dumps(bank, indent=2, ensure_ascii=False), encoding='utf-8')
        self.log(f'基因库更新完成: {bank["stats"]["total"]} 个')
        self.log('=== 进化完成 ===')
        return bank['stats']['total']

if __name__ == '__main__':
    eng = EvolutionEngine(work_dir='C:/Users/Administrator/ApexSpiral/self-evolution')
    eng.evolve()
