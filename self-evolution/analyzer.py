"""代码分析器 - 理解代码模式"""
import re

class CodeAnalyzer:
    def __init__(self):
        self.patterns = {}
    
    def analyze_file(self, code):
        """分析单个文件"""
        result = {
            'lines': code.split('\n'),
            'functions': [],
            'classes': [],
            'imports': [],
            'complexity': 0
        }
        
        # 函数检测
        for i, line in enumerate(code.split('\n')):
            if 'def ' in line and '(' in line:
                name = re.search(r'def (\w+)', line)
                if name:
                    result['functions'].append({
                        'name': name.group(1),
                        'line': i + 1
                    })
        
        # 类检测
        for line in code.split('\n'):
            if 'class ' in line:
                name = re.search(r'class (\w+)', line)
                if name:
                    result['classes'].append(name.group(1))
        
        # 导入检测
        for line in code.split('\n'):
            if 'import ' in line or 'from ' in line:
                result['imports'].append(line.strip())
        
        # 复杂度 (简单估算)
        result['complexity'] = len(result['functions']) + len(result['classes']) * 2
        
        return result
    
    def compare(self, original, generated):
        """比较原版和生成版"""
        return {
            'original_functions': len(original.get('functions', [])),
            'generated_functions': len(generated.get('functions', [])),
            'similarity': 'high' if original.get('functions') == generated.get('functions') else 'low'
        }
