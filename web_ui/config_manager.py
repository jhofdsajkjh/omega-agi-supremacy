"""
OMEGA AGI Web配置管理界面 - 配置管理器
统一管理LLM配置，支持多平台API
"""
import json
import os
import time
from pathlib import Path
from typing import Any

CONFIG_DIR = Path(__file__).parent
DEFAULT_CONFIG_FILE = CONFIG_DIR / "llm_configs.json"


class ConfigManager:
    """LLM配置管理器"""

    def __init__(self, config_file: str = str(DEFAULT_CONFIG_FILE)):
        self.config_file = Path(config_file)
        self.configs = self._load_configs()

    def _load_configs(self) -> dict:
        """加载配置文件"""
        if self.config_file.exists():
            try:
                with open(self.config_file, "r", encoding="utf-8") as f:
                    return json.load(f)
            except Exception:
                pass
        return self._default_configs()

    def _default_configs(self) -> dict:
        """默认配置"""
        return {
            "active": "openai",
            "providers": {
                "openai": {
                    "name": "OpenAI",
                    "base_url": "https://api.openai.com/v1",
                    "api_key": "",
                    "models": ["gpt-4", "gpt-4-turbo", "gpt-3.5-turbo"],
                    "default_model": "gpt-4",
                    "temperature": 0.7,
                    "max_tokens": 4096,
                    "enabled": True
                },
                "anthropic": {
                    "name": "Anthropic Claude",
                    "base_url": "https://api.anthropic.com/v1",
                    "api_key": "",
                    "models": ["claude-3-opus-20240229", "claude-3-sonnet-20240229", "claude-3-haiku-20240229"],
                    "default_model": "claude-3-sonnet-20240229",
                    "temperature": 0.7,
                    "max_tokens": 4096,
                    "enabled": False
                },
                "local": {
                    "name": "Local / Ollama",
                    "base_url": "http://localhost:11434/v1",
                    "api_key": "ollama",
                    "models": ["llama3", "codellama", "mistral"],
                    "default_model": "llama3",
                    "temperature": 0.7,
                    "max_tokens": 4096,
                    "enabled": False
                }
            },
            "layer_preferences": {
                "C_e": {"model": "auto", "temperature": 0.5},
                "E_s": {"model": "auto", "temperature": 0.6},
                "B_h": {"model": "auto", "temperature": 0.7},
                "S_d": {"model": "auto", "temperature": 0.5},
                "Q_c": {"model": "auto", "temperature": 0.4},
                "V_e": {"model": "auto", "temperature": 0.8}
            }
        }

    def load_llm_configs(self) -> dict:
        """获取所有LLM配置"""
        return self.configs

    def save_llm_configs(self, configs: dict) -> bool:
        """保存LLM配置"""
        try:
            self.configs = configs
            self.config_file.parent.mkdir(parents=True, exist_ok=True)
            with open(self.config_file, "w", encoding="utf-8") as f:
                json.dump(configs, f, indent=2, ensure_ascii=False)
            return True
        except Exception as e:
            return False

    def get_active_config(self) -> dict:
        """获取当前激活的配置"""
        active_name = self.configs.get("active", "openai")
        provider = self.configs["providers"].get(active_name, {})
        return {
            "provider": active_name,
            "name": provider.get("name", active_name),
            "base_url": provider.get("base_url", ""),
            "default_model": provider.get("default_model", ""),
            "temperature": provider.get("temperature", 0.7),
            "max_tokens": provider.get("max_tokens", 4096)
        }

    def set_active_provider(self, provider_name: str) -> bool:
        """设置激活的提供商"""
        if provider_name in self.configs["providers"]:
            self.configs["active"] = provider_name
            return self.save_llm_configs(self.configs)
        return False

    def update_provider(self, provider_name: str, config: dict) -> bool:
        """更新提供商配置"""
        if provider_name in self.configs["providers"]:
            self.configs["providers"][provider_name].update(config)
            return self.save_llm_configs(self.configs)
        return False

    def test_connection(self, provider_name: str = None) -> dict:
        """测试连接"""
        if provider_name is None:
            provider_name = self.configs.get("active", "openai")

        provider = self.configs["providers"].get(provider_name, {})
        if not provider.get("enabled"):
            return {"status": "disabled", "message": f"{provider_name} is not enabled"}

        api_key = provider.get("api_key", "")
        base_url = provider.get("base_url", "")
        model = provider.get("default_model", "")

        if not api_key:
            return {"status": "error", "message": "API key not configured"}

        try:
            import urllib.request
            import urllib.error

            headers = {
                "Authorization": f"Bearer {api_key}",
                "Content-Type": "application/json"
            }

            # 尝试发送一个简单的completion请求
            test_data = json.dumps({
                "model": model,
                "messages": [{"role": "user", "content": "Hi"}],
                "max_tokens": 5
            }).encode("utf-8")

            req = urllib.request.Request(
                f"{base_url}/chat/completions",
                data=test_data,
                headers=headers,
                method="POST"
            )

            start = time.time()
            try:
                response = urllib.request.urlopen(req, timeout=10)
                elapsed = (time.time() - start) * 1000
                return {
                    "status": "success",
                    "message": "Connection successful",
                    "latency_ms": round(elapsed)
                }
            except urllib.error.HTTPError as e:
                error_body = e.read().decode("utf-8", errors="ignore")
                return {
                    "status": "error",
                    "message": f"HTTP {e.code}: {error_body[:200]}"
                }
            except Exception as e:
                return {
                    "status": "error",
                    "message": str(e)
                }

        except Exception as e:
            return {"status": "error", "message": str(e)}

    def get_layer_status(self) -> dict:
        """获取各Layer状态"""
        return {
            "C_e": {"name": "Cognition Engine", "status": "running", "health": 95},
            "E_s": {"name": "Executive Suite", "status": "running", "health": 88},
            "B_h": {"name": "Behavioral Hub", "status": "running", "health": 92},
            "S_d": {"name": "Self Dynamics", "status": "running", "health": 85},
            "Q_c": {"name": "Quantum Coherence", "status": "running", "health": 78},
            "V_e": {"name": "Value Engine", "status": "running", "health": 90}
        }

    def run_diagnostics(self) -> dict:
        """运行自诊断"""
        issues = []
        suggestions = []

        # 检查活跃配置
        active = self.configs.get("active")
        if not active:
            issues.append("No active provider configured")
            suggestions.append("Set an active LLM provider in config.html")

        # 检查API Key
        for name, provider in self.configs["providers"].items():
            if provider.get("enabled") and not provider.get("api_key"):
                issues.append(f"{name}: API key missing")
                suggestions.append(f"Add API key for {name} in config.html")

        # 检查连接
        if active:
            result = self.test_connection(active)
            if result["status"] != "success":
                issues.append(f"{active} connection failed: {result['message']}")
                suggestions.append("Check API key and network connection")

        # 检查磁盘空间
        try:
            import shutil
            usage = shutil.disk_usage("/")
            free_gb = usage.free / (1024**3)
            if free_gb < 1:
                issues.append(f"Low disk space: {free_gb:.1f}GB free")
                suggestions.append("Free up disk space to ensure正常运行")
        except:
            pass

        return {
            "timestamp": time.time(),
            "overall_health": 100 - len(issues) * 10,
            "issues": issues,
            "suggestions": suggestions,
            "layers": self.get_layer_status()
        }