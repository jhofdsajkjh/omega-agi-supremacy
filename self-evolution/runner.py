import pathlib
"""演进运行器 - 定时执行和学习闭环"""
import subprocess, sys
from datetime import datetime

class EvolutionRunner:
    def __init__(self, token, output_dir):
        self.token = token
        self.output_dir = pathlib.Path(output_dir)
        self.log_file = self.output_dir / 'evolution.log'
        self.results_dir = self.output_dir / 'results'
        self.results_dir.mkdir(parents=True, exist_ok=True)
    
    def run(self):
        """执行一轮演进"""
        log = []
        log.append(f"\n=== Evolution Run {datetime.now().isoformat()} ===")
        
        # 执行 fetcher
        cmd = [
            sys.executable, 
            str(self.output_dir / 'fetcher.py'),
            self.token,
            str(self.results_dir)
        ]
        
        try:
            proc = subprocess.run(cmd, capture_output=True, text=True, timeout=300)
            log.append(proc.stdout)
            if proc.returncode == 0:
                log.append("✓ 演进完成")
            else:
                log.append(f"✗ 错误: {proc.stderr}")
        except Exception as e:
            log.append(f"✗ 异常: {e}")
        
        output = '\n'.join(log)
        
        # 追加日志
        with open(self.log_file, 'a', encoding='utf-8') as f:
            f.write(output)
        
        return output


if __name__ == '__main__':
    import sys
    token = sys.argv[1] if len(sys.argv) > 1 else None
    output = sys.argv[2] if len(sys.argv) > 2 else './evolution'
    
    if token:
        runner = EvolutionRunner(token, output)
        print(runner.run())
