import pathlib
"""知识存储 - 基因库管理"""
import json
from datetime import datetime

class GeneStorage:
    def __init__(self, base_dir):
        self.base_dir = pathlib.Path(base_dir)
        self.genes_file = self.base_dir / 'genes.json'
        self.history_file = self.base_dir / 'history.json'
        self.load()
    
    def load(self):
        """加载基因库"""
        if self.genes_file.exists():
            self.genes = json.loads(self.genes_file.read_text(encoding='utf-8'))
        else:
            self.genes = {'by_type': {}, 'by_source': {}}
        
        if self.history_file.exists():
            self.history = json.loads(self.history_file.read_text(encoding='utf-8'))
        else:
            self.history = []
    
    def save(self):
        """保存基因库"""
        self.genes_file.write_text(json.dumps(self.genes, indent=2, ensure_ascii=False), encoding='utf-8')
        self.history_file.write_text(json.dumps(self.history, indent=2, ensure_ascii=False), encoding='utf-8')
    
    def add_gene(self, gene_type, pattern, source):
        """添加基因"""
        if gene_type not in self.genes['by_type']:
            self.genes['by_type'][gene_type] = []
        
        gene = {
            'pattern': pattern,
            'source': source,
            'added': datetime.now().isoformat()
        }
        
        # 检查是否已存在
        exists = any(g['pattern'] == pattern for g in self.genes['by_type'][gene_type])
        if not exists:
            self.genes['by_type'][gene_type].append(gene)
            self.history.append(gene)
            self.save()
            return True
        return False
    
    def get_genes(self, gene_type=None):
        """获取基因"""
        if gene_type:
            return self.genes['by_type'].get(gene_type, [])
        return self.genes['by_type']
    
    def search(self, query):
        """搜索基因"""
        results = []
        for gene_type, genes in self.genes['by_type'].items():
            for g in genes:
                if query.lower() in g['pattern'].lower():
                    results.append({**g, 'type': gene_type})
        return results
