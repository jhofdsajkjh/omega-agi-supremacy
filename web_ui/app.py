"""
OMEGA AGI Web配置管理界面 - Flask主应用
提供LLM配置、系统状态、健康检查和自诊断API
"""
import sys
import os

# 添加父目录到路径以便导入config_manager
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

from flask import Flask, render_template, request, jsonify
import json
import psutil
import time
import platform
from config_manager import ConfigManager

app = Flask(__name__)
app.config['JSON_AS_ASCII'] = False

# 初始化配置管理器
config_manager = ConfigManager()


# ============== 主页面路由 ==============

@app.route('/')
def index():
    """主仪表盘"""
    return render_template('index.html')


@app.route('/config')
def config_page():
    """LLM配置页面"""
    return render_template('config.html')


@app.route('/status')
def status_page():
    """系统状态页面"""
    return render_template('status.html')


@app.route('/diagnostics')
def diagnostics_page():
    """自诊断面板页面"""
    return render_template('diagnostics.html')


# ============== LLM配置API ==============

@app.route('/api/config/llm', methods=['GET', 'POST'])
def llm_config():
    """获取或保存LLM配置"""
    if request.method == 'GET':
        configs = config_manager.load_llm_configs()
        # 隐藏API key
        for name, provider in configs.get("providers", {}).items():
            if provider.get("api_key"):
                provider["api_key"] = mask_api_key(provider["api_key"])
        return jsonify({"success": True, "data": configs})

    elif request.method == 'POST':
        data = request.get_json()
        if not data:
            return jsonify({"success": False, "error": "No data provided"})

        configs = config_manager.load_llm_configs()

        # 更新配置
        if "active" in data:
            configs["active"] = data["active"]
        if "providers" in data:
            for name, provider in data["providers"].items():
                if name in configs["providers"]:
                    # 保留真实API key
                    if not provider.get("api_key") or provider["api_key"].startswith("***"):
                        provider["api_key"] = configs["providers"][name].get("api_key", "")
                    configs["providers"][name].update(provider)
        if "layer_preferences" in data:
            configs["layer_preferences"] = data["layer_preferences"]

        if config_manager.save_llm_configs(configs):
            return jsonify({"success": True, "message": "Configuration saved"})
        else:
            return jsonify({"success": False, "error": "Failed to save configuration"})


@app.route('/api/config/active', methods=['GET', 'POST'])
def active_config():
    """获取或设置当前激活的提供商"""
    if request.method == 'GET':
        return jsonify({"success": True, "data": config_manager.get_active_config()})

    elif request.method == 'POST':
        data = request.get_json()
        provider_name = data.get("provider")
        if not provider_name:
            return jsonify({"success": False, "error": "Provider name required"})

        if config_manager.set_active_provider(provider_name):
            return jsonify({"success": True, "message": f"Active provider set to {provider_name}"})
        else:
            return jsonify({"success": False, "error": "Failed to set active provider"})


@app.route('/api/config/test', methods=['POST'])
def test_connection():
    """测试连接"""
    data = request.get_json()
    provider_name = data.get("provider") if data else None
    result = config_manager.test_connection(provider_name)
    return jsonify({"success": result["status"] == "success", "data": result})


# ============== 系统状态API ==============

@app.route('/api/status')
def system_status():
    """获取系统状态"""
    try:
        layers = config_manager.get_layer_status()

        # CPU使用率
        cpu_percent = psutil.cpu_percent(interval=0.1)

        # 内存使用率
        memory = psutil.virtual_memory()
        memory_percent = memory.percent

        # 磁盘使用率
        disk = psutil.disk_usage('/')

        # 网络状态
        net_io = psutil.net_io_counters()

        return jsonify({
            "success": True,
            "data": {
                "timestamp": time.time(),
                "platform": {
                    "system": platform.system(),
                    "release": platform.release(),
                    "machine": platform.machine()
                },
                "layers": layers,
                "resources": {
                    "cpu": {
                        "percent": cpu_percent,
                        "count": psutil.cpu_count()
                    },
                    "memory": {
                        "percent": memory_percent,
                        "total_gb": round(memory.total / (1024**3), 1),
                        "used_gb": round(memory.used / (1024**3), 1),
                        "available_gb": round(memory.available / (1024**3), 1)
                    },
                    "disk": {
                        "percent": disk.percent,
                        "total_gb": round(disk.total / (1024**3), 1),
                        "free_gb": round(disk.free / (1024**3), 1)
                    },
                    "network": {
                        "bytes_sent": net_io.bytes_sent,
                        "bytes_recv": net_io.bytes_recv
                    }
                }
            }
        })
    except Exception as e:
        return jsonify({"success": False, "error": str(e)})


# ============== 健康检查API ==============

@app.route('/api/health')
def health_check():
    """健康检查"""
    status = "healthy"
    issues = []

    # 检查CPU
    cpu = psutil.cpu_percent(interval=0.1)
    if cpu > 90:
        status = "degraded"
        issues.append(f"High CPU usage: {cpu}%")

    # 检查内存
    memory = psutil.virtual_memory()
    if memory.percent > 90:
        status = "degraded"
        issues.append(f"High memory usage: {memory.percent}%")

    # 检查磁盘
    disk = psutil.disk_usage('/')
    if disk.percent > 90:
        status = "degraded"
        issues.append(f"Low disk space: {100-disk.percent}% free")

    # 检查LLM连接
    active = config_manager.get_active_config()
    if active.get("default_model"):
        result = config_manager.test_connection()
        if result["status"] != "success":
            status = "degraded"
            issues.append(f"LLM connection failed")

    return jsonify({
        "success": True,
        "data": {
            "status": status,
            "timestamp": time.time(),
            "issues": issues
        }
    })


# ============== 自诊断API ==============

@app.route('/api/diagnostics')
def diagnostics():
    """自诊断报告"""
    result = config_manager.run_diagnostics()
    return jsonify({"success": True, "data": result})


@app.route('/api/diagnostics/fix', methods=['POST'])
def apply_fix():
    """应用修复建议"""
    data = request.get_json()
    fix_type = data.get("type") if data else None

    results = []

    # 根据修复类型执行相应操作
    if fix_type == "clear_cache":
        try:
            import shutil
            cache_dirs = ["/tmp/openclaw", "/root/.cache"]
            for d in cache_dirs:
                if os.path.exists(d):
                    shutil.rmtree(d)
                    os.makedirs(d, exist_ok=True)
            results.append({"action": "clear_cache", "status": "success"})
        except Exception as e:
            results.append({"action": "clear_cache", "status": "error", "message": str(e)})

    elif fix_type == "restart_services":
        results.append({"action": "restart_services", "status": "info", "message": "Service restart not implemented in web mode"})

    else:
        results.append({"action": "unknown", "status": "error", "message": f"Unknown fix type: {fix_type}"})

    return jsonify({"success": True, "data": {"results": results}})


# ============== 工具函数 ==============

def mask_api_key(key: str, visible: int = 4) -> str:
    """掩码API key"""
    if not key or len(key) <= visible:
        return "***"
    return key[:visible] + "***" + key[-visible:]


# ============== 启动 ==============

if __name__ == '__main__':
    print("=" * 60)
    print("OMEGA AGI Web配置管理界面")
    print("=" * 60)
    print("访问 http://localhost:5000")
    print("页面:")
    print("  /         - 主仪表盘")
    print("  /config   - LLM配置")
    print("  /status  - 系统状态")
    print("  /diagnostics - 自诊断")
    print("=" * 60)
    app.run(host='0.0.0.0', port=5000, debug=True)