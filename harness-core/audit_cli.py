#!/usr/bin/env python3
"""
Life-Harness Audit CLI Wrapper
用法: python audit_cli.py <file_or_gene_text>
"""

import sys, pathlib
sys.path.insert(0, str(pathlib.Path(__file__).parent))

from audit_engine import audit_file, audit_gene, full_audit

if __name__ == "__main__":
    import json
    
    if len(sys.argv) < 2:
        print("用法: python audit_cli.py <file_path_or_gene_text>")
        sys.exit(1)
    
    arg = sys.argv[1]
    p = pathlib.Path(arg)
    
    if p.exists():
        result = audit_file(arg)
    else:
        result = audit_gene(arg)
    
    print(json.dumps(result, ensure_ascii=False, indent=2))