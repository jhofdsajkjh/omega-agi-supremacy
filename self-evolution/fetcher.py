"""代码抓取器 - 从 GitHub 获取优秀代码"""
import pathlib, urllib.request, json, base64
from datetime import datetime

class CodeFetcher:
    def __init__(self, token):
        self.token = token
        self.headers = {
            'Authorization': f'Bearer {token}',
            'Accept': 'application/vnd.github+json',
            'User-Agent': 'Hermes-Evolution'
        }
    
    def get_trending(self, lang='python', limit=10):
        """获取 GitHub Trending"""
        url = f'https://api.github.com/search/repositories?q=stars:>10000+language:{lang}&sort=stars&order=desc'
        req = urllib.request.Request(url, headers=self.headers)
        with urllib.request.urlopen(req, timeout=30) as r:
            data = json.loads(r.read().decode())
        return data.get('items', [])[:limit]
    
    def get_repo_contents(self, owner, repo, path=''):
        """获取仓库内容列表"""
        url = f'https://api.github.com/repos/{owner}/{repo}/contents/{path}'
        req = urllib.request.Request(url, headers=self.headers)
        with urllib.request.urlopen(req, timeout=30) as r:
            return json.loads(r.read().decode())
    
    def get_file_content(self, url):
        """通过 URL 获取文件内容"""
        req = urllib.request.Request(url, headers=self.headers)
        with urllib.request.urlopen(req, timeout=30) as r:
            data = json.loads(r.read().decode())
        if isinstance(data, dict) and data.get('content'):
            return base64.b64decode(data['content']).decode('utf-8')
        return None
    
    def search_code(self, query, lang='python'):
        """搜索代码"""
        url = f'https://api.github.com/search/code?q={query}+language:{lang}'
        req = urllib.request.Request(url, headers=self.headers)
        with urllib.request.urlopen(req, timeout=30) as r:
            return json.loads(r.read().decode())


class GeneExtractor:
    """基因提取器"""
    
    @staticmethod
    def extract(code):
        """从代码中提取基因"""
        genes = []
        lines = code.split('\n')
        
        for line in lines:
            line = line.strip()
            if not line or line.startswith('#'):
                continue
            
            # 函数
            if 'def ' in line and '(' in line and ':' in line:
                genes.append(('function', line))
            # 类
            elif 'class ' in line and ':' in line:
                genes.append(('class', line))
            # 导入
            elif 'import ' in line or 'from ' in line:
                genes.append(('import', line))
            # 装饰器
            elif line.startswith('@') and 'def ' not in line:
                genes.append(('decorator', line))
        
        return genes
    
    @staticmethod
    def analyze(code):
        """分析代码结构"""
        return {
            'lines': len(code.split('\n')),
            'functions': len([l for l in code.split('\n') if 'def ' in l and '(' in l]),
            'classes': len([l for l in code.split('\n') if 'class ' in l]),
            'imports': len([l for l in code.split('\n') if 'import ' in l or 'from ' in l]),
        }


class Evolver:
    """演进器"""
    
    def __init__(self, token, output_dir):
        self.fetcher = CodeFetcher(token)
        self.output_dir = pathlib.Path(output_dir)
        self.output_dir.mkdir(parents=True, exist_ok=True)
        self.genes_file = self.output_dir / 'genes.json'
        self.log_file = self.output_dir / 'evolution.log'
        self.load_genes()
    
    def load_genes(self):
        """加载已有基因"""
        if self.genes_file.exists():
            self.genes = json.loads(self.genes_file.read_text(encoding='utf-8'))
        else:
            self.genes = {'by_type': {}, 'history': []}
    
    def save_genes(self):
        """保存基因"""
        self.genes_file.write_text(json.dumps(self.genes, indent=2, ensure_ascii=False), encoding='utf-8')
    
    def add_gene(self, gene_type, pattern, source):
        """添加基因"""
        if gene_type not in self.genes['by_type']:
            self.genes['by_type'][gene_type] = []
        
        # 检查是否已存在
        exists = any(g['pattern'] == pattern for g in self.genes['by_type'][gene_type])
        if not exists:
            gene = {
                'pattern': pattern,
                'source': source,
                'added': datetime.now().isoformat()
            }
            self.genes['by_type'][gene_type].append(gene)
            self.genes['history'].append(gene)
            return True
        return False
    
    def evolve(self):
        """执行一轮演进"""
        log = []
        log.append(f"\n[{datetime.now().isoformat()}] 开始演进")
        
        # 获取 trending
        log.append("获取 GitHub Trending Python...")
        repos = self.fetcher.get_trending('python', 5)
        
        new_genes = []
        for repo in repos:
            name = repo.get('full_name')
            stars = repo.get('stargazers_count')
            log.append(f"  仓库: {name} ⭐{stars}")
            
            owner, repo_name = name.split('/')
            
            try:
                # 获取目录内容
                contents = self.fetcher.get_repo_contents(owner, repo_name)
                
                # 找子目录
                dirs = [c for c in contents if c.get('type') == 'dir'][:2]
                
                for d in dirs:
                    sub_contents = self.fetcher.get_repo_contents(owner, repo_name, d['name'])
                    
                    # 找 Python 文件
                    py_files = [c for c in sub_contents if c.get('name', '').endswith('.py')][:2]
                    
                    for pf in py_files:
                        code = self.fetcher.get_file_content(pf['url'])
                        if code and len(code) > 100:
                            genes = GeneExtractor.extract(code)
                            stats = GeneExtractor.analyze(code)
                            log.append(f"    {pf['name']}: {stats['functions']}个函数, {stats['classes']}个类")
                            
                            # 添加基因
                            for gtype, pattern in genes:
                                if self.add_gene(gtype, pattern, name):
                                    new_genes.append((gtype, pattern[:50]))
            except Exception as e:
                log.append(f"    跳过: {e}")
        
        self.save_genes()
        
        log.append(f"\n✓ 演进完成: 新增 {len(new_genes)} 个基因")
        
        output = '\n'.join(log)
        
        # 写入日志
        with open(self.log_file, 'a', encoding='utf-8') as f:
            f.write(output)
        
        return output


if __name__ == '__main__':
    import sys
    token = sys.argv[1] if len(sys.argv) > 1 else None
    output = sys.argv[2] if len(sys.argv) > 2 else './evolution'
    
    if token:
        evolver = Evolver(token, output)
        print(evolver.evolve())
