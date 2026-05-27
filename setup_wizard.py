#!/usr/bin/env python3
"""
OMEGA AGI Supremacy - 交互式配置向导
图形化引导用户完成LLM配置、服务配置、启动确认
"""
import os
import sys
import json
import readline
from pathlib import Path
from typing import Optional, Tuple

try:
    from rich.console import Console
    from rich.panel import Panel
    from rich.prompt import Prompt, Confirm
    from rich.table import Table
    from rich.progress import Progress, SpinnerColumn, TextColumn
    RICH_AVAILABLE = True
except ImportError:
    RICH_AVAILABLE = False

# ============================================================================
# 常量
# ============================================================================
ROBOT = "🤖"
ROCKET = "🚀"
CHECK = "✅"
CROSS = "❌"
WARN = "⚠️"
INFO = "ℹ️"
FOLDER = "📁"
GEAR = "🔧"
SPARKLES = "✨"
GLOBE = "🌐"

PROJECT_DIR = Path(__file__).parent
CONFIG_FILE = PROJECT_DIR / ".env"
LLM_CONFIGS_FILE = PROJECT_DIR / "web_ui" / "llm_configs.json"

# LLM 提供商
LLM_PROVIDERS = {
    "1": {
        "name": "OpenAI",
        "url": "https://api.openai.com/v1/chat/completions",
        "default_model": "gpt-4o",
        "key_hint": "sk-...",
        "docs": "https://platform.openai.com/api-keys",
        "color": "green",
    },
    "2": {
        "name": "Anthropic",
        "url": "https://api.anthropic.com/v1/messages",
        "default_model": "claude-sonnet-4-20250514",
        "key_hint": "sk-ant-...",
        "docs": "https://console.anthropic.com/api-keys",
        "color": "red",
    },
    "3": {
        "name": "Groq",
        "url": "https://api.groq.com/openai/v1/chat/completions",
        "default_model": "llama-3.3-70b-versatile",
        "key_hint": "gsk_...",
        "docs": "https://console.groq.com/api-keys",
        "color": "cyan",
    },
    "4": {
        "name": "DeepSeek",
        "url": "https://api.deepseek.com/v1/chat/completions",
        "default_model": "deepseek-chat",
        "key_hint": "sk-...",
        "docs": "https://platform.deepseek.com/api_keys",
        "color": "blue",
    },
    "5": {
        "name": "Ollama (本地)",
        "url": "http://localhost:11434/v1/chat/completions",
        "default_model": "llama3",
        "key_hint": "ollama-local",
        "docs": "https://github.com/ollama/ollama",
        "color": "magenta",
    },
    "6": {
        "name": "硅基流动",
        "url": "https://api.siliconflow.cn/v1/chat/completions",
        "default_model": "Qwen/Qwen2.5-72B-Instruct",
        "key_hint": "sk-...",
        "docs": "https://www.siliconflow.cn",
        "color": "yellow",
    },
    "7": {
        "name": "自定义",
        "url": "",
        "default_model": "",
        "key_hint": "",
        "docs": "",
        "color": "white",
    },
}

# ============================================================================
# Rich Console 包装器
# ============================================================================
class RichUI:
    def __init__(self):
        self.console = Console() if RICH_AVAILABLE else None

    def print(self, text="", style=None, emoji=None):
        prefix = f"{emoji}  " if emoji else "   "
        if self.console:
            self.console.print(prefix + text)
        else:
            print(f"{prefix}{text}")

    def panel(self, title, content, style="cyan"):
        if self.console:
            self.console.print(Panel(content, title=title, border_style=style))
        else:
            print(f"\n=== {title} ===\n{content}\n")

    def table(self, headers, rows):
        if not self.console:
            for i, h in enumerate(headers):
                print(f"{h:20}", end="")
            print()
            for row in rows:
                for cell in row:
                    print(f"{str(cell):20}", end="")
                print()
            return
        table = Table(show_header=True, header_style="bold cyan")
        for h in headers:
            table.add_column(h, style="cyan")
        for row in rows:
            table.add_row(*[str(c) for c in row])
        self.console.print(table)

    def prompt(self, question, choices=None, default=None, password=False):
        if self.console:
            kwargs = {"prompt": f"\n{question}: "}
            if choices:
                kwargs["choices"] = [str(c) for c in choices]
            if default:
                kwargs["default"] = str(default)
            if password:
                kwargs["password"] = True
            return Prompt.ask(**kwargs)
        else:
            prompt_text = f"\n{question}"
            if choices:
                opts = "/".join(str(c) for c in choices)
                prompt_text += f" [{opts}]"
            if default:
                prompt_text = prompt_text.replace("[", f"[默认: {default}")
            answer = input(prompt_text + ": ").strip()
            return answer or str(default) if default else answer

    def confirm(self, question, default=True):
        if self.console:
            return Confirm.ask(f"\n{question}", default=default)
        else:
            suffix = "[Y/n]" if default else "[y/N]"
            answer = input(f"{question} {suffix}: ").strip().lower()
            if not answer:
                return default
            return answer in ["y", "yes", "是"]

    def progress(self, description):
        if self.console:
            return Progress(
                SpinnerColumn(),
                TextColumn("[progress.description]{task.description}"),
                console=self.console,
            )
        return DummyProgress()

class DummyProgress:
    def __enter__(self):
        return self
    def __exit__(self, *args):
        pass
    def add_task(self, description, total=None):
        return 0
    def update(self, task_id, advance=None, **kwargs):
        pass

ui = RichUI()

# ============================================================================
# 工具函数
# ============================================================================
def clear_screen():
    os.system("cls" if os.name == "nt" else "clear")

def separator():
    print("─" * 48)

def read_env(key, default=""):
    """从 .env 文件读取值"""
    if not CONFIG_FILE.exists():
        return default
    with open(CONFIG_FILE, "r") as f:
        for line in f:
            line = line.strip()
            if line.startswith("#") or not line:
                continue
            if "=" in line:
                k, v = line.split("=", 1)
                if k == key:
                    return v.strip()
    return default

def write_env(key, value, comment=None):
    """写入 .env 文件"""
    lines = []
    if CONFIG_FILE.exists():
        with open(CONFIG_FILE, "r") as f:
            lines = f.readlines()

    new_lines = []
    key_found = False
    for line in lines:
        stripped = line.strip()
        if stripped.startswith("#") or not stripped:
            new_lines.append(line)
            continue
        if "=" in line:
            k, _ = stripped.split("=", 1)
            if k == key:
                new_lines.append(f"{key}={value}\n")
                key_found = True
                continue
        new_lines.append(line)

    if not key_found:
        if comment:
            new_lines.append(f"# {comment}\n")
        new_lines.append(f"{key}={value}\n")

    with open(CONFIG_FILE, "w") as f:
        f.writelines(new_lines)

def mask_key(key):
    """脱敏显示 API Key"""
    if not key or len(key) < 8:
        return "****"
    return key[:4] + "****" + key[-4:]

def print_banner():
    banner = f"""
{ROBOT}  OMEGA AGI Supremacy - 配置向导  {ROBOT}
{'━' * 44}
     The Most Powerful Autonomous AI Agent Framework
                    Interactive Setup Wizard
"""
    print(banner)

# ============================================================================
# 欢迎步骤
# ============================================================================
def step_welcome():
    clear_screen()
    print_banner()
    ui.panel(
        "欢迎使用 OMEGA AGI Supremacy",
        f"""这个向导将帮助你完成以下配置：

  {INFO} LLM API 连接配置
  {INFO} Web UI 服务选项
  {INFO} 健康检查与监控
  {INFO} 启动与验证

  整个过程大约需要 3-5 分钟。
  随时按 Ctrl+C 退出。
""",
        style="cyan"
    )

    if not ui.confirm("是否开始配置？", default=True):
        print("\n已取消。再见！👋")
        sys.exit(0)

# ============================================================================
# LLM 配置步骤
# ============================================================================
def step_llm_config():
    clear_screen()
    print_banner()

    ui.panel("第一步：LLM 提供商配置", style="green")
    print()

    # 显示已有配置
    current_provider = read_env("OMEGA_LLM_PROVIDER", "")
    current_key = read_env("OMEGA_API_KEY", "")
    current_url = read_env("OMEGA_API_URL", "")
    current_model = read_env("OMEGA_MODEL_NAME", "")

    if current_key:
        print(f"  {INFO} 当前配置: {mask_key(current_key)} / {current_model or '未设置模型'}")

    print("  请选择 LLM 提供商：\n")

    # 表格展示提供商
    headers = ["选项", "提供商", "默认模型", "特点"]
    rows = [
        ["1", "OpenAI", "gpt-4o", "最强大，付费"],
        ["2", "Anthropic", "Claude 3 Sonnet", "高质量推理"],
        ["3", "Groq", "Llama-3.3-70B", "免费高速 ⭐推荐"],
        ["4", "DeepSeek", "deepseek-chat", "深度推理，便宜"],
        ["5", "Ollama", "llama3", "本地运行，免费"],
        ["6", "硅基流动", "Qwen2.5-72B", "中文优化"],
        ["7", "自定义", "—", "手动输入 URL 和 Key"],
    ]
    ui.table(headers, rows)
    print()

    choice = ui.prompt(
        "请选择 LLM 提供商",
        choices=[str(i) for i in range(1, 8)],
        default="3"
    )

    if choice == "7":
        custom_url = ui.prompt("请输入自定义 API URL")
        custom_key = ui.prompt("请输入 API Key", password=True)
        custom_model = ui.prompt("请输入模型名称")
        provider_info = {
            "name": "自定义",
            "url": custom_url,
            "default_model": custom_model,
            "key": custom_key,
        }
    else:
        provider_info = LLM_PROVIDERS.get(choice, LLM_PROVIDERS["3"]).copy()
        provider_info["key"] = ui.prompt(
            f"请输入 {provider_info['name']} API Key",
            password=True,
        )
        if choice == "5":
            provider_info["key"] = "ollama-local"

        if provider_info["key"] == "ollama-local":
            pass
        elif not provider_info["key"]:
            print(f"\n  {WARN} 未提供 API Key，将使用环境变量")
        else:
            print(f"\n  {CHECK} API Key 已输入")

        # 确认/修改模型
        print(f"\n  默认模型: {provider_info['default_model']}")
        model = ui.prompt("确认或修改模型名称", default=provider_info["default_model"])
        provider_info["default_model"] = model or provider_info["default_model"]

    return provider_info, choice

# ============================================================================
# GitHub Token 配置
# ============================================================================
def step_github_token():
    clear_screen()
    print_banner()

    ui.panel("第二步：GitHub Token (可选)", style="yellow")
    print()

    print("  配置 GitHub Token 可以解锁：")
    print("    • 访问私有仓库")
    print("    • 更高的 API 速率限制")
    print("    • 自动项目同步")
    print()
    print(f"  获取地址: {GLOBE} https://github.com/settings/tokens")
    print()

    current_token = read_env("GITHUB_TOKEN", "")
    if current_token:
        print(f"  {INFO} 当前已配置 Token: {mask_key(current_token)}")

    token = ui.prompt("输入 GitHub Token (直接回车跳过)", password=True)
    token = token.strip() if token else ""

    return token if token else None

# ============================================================================
# 服务配置
# ============================================================================
def step_service_config():
    clear_screen()
    print_banner()

    ui.panel("第三步：服务配置", style="cyan")
    print()

    print("  选择启动模式：\n")
    print("    1️⃣  开发模式  - Web UI + 热重载，适合调试")
    print("    2️⃣  生产模式  - 高性能，适合正式部署")
    print("    3️⃣  仅 API    - 无 Web UI，纯后台服务")
    print()

    mode = ui.prompt("选择启动模式", choices=["1", "2", "3"], default="1")
    mode_names = {"1": "development", "2": "production", "3": "api_only"}
    mode_name = mode_names.get(mode, "development")

    # Web 端口配置
    print()
    current_port = read_env("OMEGA_WEB_PORT", "5000")
    port = ui.prompt(f"Web UI 端口 [默认 {current_port}]", default=current_port)

    return mode_name, port

# ============================================================================
# 验证与保存
# ============================================================================
def step_verify_and_save(provider_info, github_token, mode, port):
    clear_screen()
    print_banner()

    ui.panel("第四步：确认配置", style="green")
    print()

    # 显示配置摘要
    table_data = [
        ["LLM 提供商", provider_info["name"]],
        ["API URL", provider_info["url"] or "(使用默认)"],
        ["模型名称", provider_info["default_model"]],
        ["API Key", mask_key(provider_info.get("key", ""))],
        ["启动模式", mode],
        ["Web 端口", port],
        ["GitHub Token", mask_key(github_token) if github_token else "未配置"],
    ]
    ui.table(["配置项", "值"], table_data)
    print()

    return ui.confirm("确认以上配置无误？", default=True)

# ============================================================================
# 保存配置
# ============================================================================
def save_config(provider_info, github_token, mode, port):
    clear_screen()
    print_banner()

    ui.panel("正在保存配置...", style="cyan")
    print()

    with ui.progress("保存配置文件") as progress:
        task = progress.add_task("", total=100)

        # 保存 .env
        write_env("OMEGA_LLM_PROVIDER", provider_info.get("choice_num", "3"), "LLM Provider")
        write_env("OMEGA_API_URL", provider_info.get("url", ""))
        write_env("OMEGA_API_KEY", provider_info.get("key", ""))
        write_env("OMEGA_MODEL_NAME", provider_info.get("default_model", ""))
        progress.update(task, advance=30)

        write_env("OMEGA_STARTUP_MODE", mode)
        write_env("OMEGA_WEB_PORT", port)
        progress.update(task, advance=20)

        if github_token:
            write_env("GITHUB_TOKEN", github_token, "GitHub Token (可选)")
        progress.update(task, advance=20)

        # 保存 llm_configs.json (供 Web UI 使用)
        save_llm_configs_json(provider_info)
        progress.update(task, advance=30)

    print(f"\n  {CHECK} 配置已保存到: {CONFIG_FILE}")
    print(f"  {CHECK} Web UI 配置已保存到: {LLM_CONFIGS_FILE}")

def save_llm_configs_json(provider_info):
    """保存 llm_configs.json 供 Web UI 使用"""
    LLM_CONFIGS_FILE.parent.mkdir(parents=True, exist_ok=True)

    config = {
        "active_provider": provider_info.get("name", "Groq"),
        "providers": {
            provider_info.get("name", "Groq"): {
                "api_url": provider_info.get("url", ""),
                "api_key": provider_info.get("key", ""),
                "model": provider_info.get("default_model", "llama-3.3-70b-versatile"),
                "enabled": True,
            }
        },
        "updated_at": "",
    }

    with open(LLM_CONFIGS_FILE, "w") as f:
        json.dump(config, f, indent=2, ensure_ascii=False)

# ============================================================================
# 健康检查
# ============================================================================
def health_check(provider_info):
    clear_screen()
    print_banner()

    ui.panel("第五步：启动前健康检查", style="cyan")
    print()

    checks = []

    # 检查 Python
    checks.append(("Python 环境", sys.version.split()[0], True))

    # 检查依赖
    deps = ["openai", "requests", "flask"]
    missing = []
    for dep in deps:
        try:
            __import__(dep)
            checks.append((f"依赖 {dep}", "已安装", True))
        except ImportError:
            checks.append((f"依赖 {dep}", "未安装", False))
            missing.append(dep)

    # 检查配置文件
    checks.append(("配置文件", str(CONFIG_FILE), CONFIG_FILE.exists()))

    # 检查端口占用
    import socket
    port = int(read_env("OMEGA_WEB_PORT", "5000"))
    port_free = True
    try:
        s = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
        s.bind(("0.0.0.0", port))
        s.close()
    except OSError:
        port_free = False
        checks.append(("端口 5000", "已被占用", False))

    ui.table(["检查项", "状态", "通过"], checks)

    if missing:
        print(f"\n  {WARN} 缺少依赖: {', '.join(missing)}")
        print(f"  运行: {GEAR} pip install {' '.join(missing)}")

    if not port_free:
        print(f"\n  {WARN} 端口 {port} 已被占用，请修改端口或停止占用进程")

    print()

    return len([c for c in checks if c[2]]) == len(checks)

# ============================================================================
# 启动
# ============================================================================
def start_services():
    clear_screen()
    print_banner()

    ui.panel("🎉 配置完成！准备启动...", style="green")
    print()

    print(f"  {ROCKET} 启动服务中...\n")

    # 检查 launcher.sh
    launcher = PROJECT_DIR / "launcher.sh"
    if launcher.exists():
        os.chmod(str(launcher), 0o755)
        print(f"  {INFO} 检测到 launcher.sh，启动脚本...")
        print(f"\n  请手动运行: {GEAR} bash launcher.sh")
    else:
        # 尝试直接启动
        web_ui_main = PROJECT_DIR / "omega-agi" / "web_ui" / "app.py"
        if web_ui_main.exists():
            print(f"  {INFO} 启动 Web UI...")
            print(f"\n  请手动运行: {GEAR} python3 {web_ui_main}")
        else:
            print(f"  {WARN} 未找到启动入口，请参考 README.md 启动")

    print()

    ui.panel(
        "启动摘要",
        f"""  📍 访问地址: http://localhost:{read_env("OMEGA_WEB_PORT", "5000")}
  📝 配置文件: {CONFIG_FILE}
  🔧 配置向导: python3 setup_wizard.py
  📖 文档:     QUICKSTART.md / README.md
""",
        style="cyan"
    )

    print(f"\n  {SPARKLES} 恭喜！OMEGA AGI 已准备就绪！\n")

# ============================================================================
# 主流程
# ============================================================================
def run_setup_wizard():
    try:
        step_welcome()

        provider_info, choice_num = step_llm_config()
        provider_info["choice_num"] = choice_num

        github_token = step_github_token()

        mode, port = step_service_config()

        if not step_verify_and_save(provider_info, github_token, mode, port):
            print("\n已取消。请重新运行向导。")
            sys.exit(0)

        save_config(provider_info, github_token, mode, port)

        health_ok = health_check(provider_info)

        if health_ok:
            start_services()
        else:
            print(f"\n  {WARN} 健康检查发现问题，建议修复后再启动")
            if not ui.confirm("是否仍要尝试启动？", default=False):
                print("\n已退出。请修复问题后重新运行向导。")
                sys.exit(1)
            start_services()

    except KeyboardInterrupt:
        print("\n\n已中断。再见！👋")
        sys.exit(0)

if __name__ == "__main__":
    run_setup_wizard()