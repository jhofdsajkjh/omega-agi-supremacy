/**
 * OMEGA AGI Web UI - 前端逻辑
 */

// API 基础路径
const API_BASE = '';

// 工具函数
function showToast(message, type = 'info') {
    let toast = document.getElementById('toast');
    if (!toast) {
        toast = document.createElement('div');
        toast.id = 'toast';
        toast.className = 'toast';
        document.body.appendChild(toast);
    }
    toast.textContent = message;
    toast.className = `toast toast-${type} show`;
    setTimeout(() => { toast.className = 'toast'; }, 3000);
}

async function apiRequest(endpoint, options = {}) {
    try {
        const res = await fetch(API_BASE + endpoint, {
            headers: {
                'Content-Type': 'application/json',
                ...options.headers
            },
            ...options
        });
        return await res.json();
    } catch (e) {
        console.error('API Error:', e);
        return { success: false, error: e.message };
    }
}

// 格式化字节
function formatBytes(bytes) {
    if (bytes === 0) return '0 B';
    const k = 1024;
    const sizes = ['B', 'KB', 'MB', 'GB', 'TB'];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return parseFloat((bytes / Math.pow(k, i)).toFixed(2)) + ' ' + sizes[i];
}

// 格式化时间戳
function formatTimestamp(timestamp) {
    if (!timestamp) return '--';
    const date = new Date(timestamp * 1000);
    return date.toLocaleString();
}

// 获取健康状态颜色
function getHealthColor(health) {
    if (health > 80) return '#48c78e';
    if (health > 60) return '#ff9f43';
    return '#ff4757';
}

// Layer颜色映射
const LAYER_COLORS = {
    'C_e': '#667eea',
    'E_s': '#764ba2',
    'B_h': '#f093fb',
    'S_d': '#f5576c',
    'Q_c': '#4facfe',
    'V_e': '#43e97b'
};

// 页面初始化
document.addEventListener('DOMContentLoaded', () => {
    // 添加fade-in动画到卡片
    document.querySelectorAll('.card').forEach((card, i) => {
        card.classList.add('fade-in');
        card.style.animationDelay = `${i * 0.1}s`;
    });
});

// 导航高亮
function setActiveNav(currentPage) {
    document.querySelectorAll('.nav a').forEach(link => {
        link.classList.remove('active');
        if (link.getAttribute('href') === currentPage) {
            link.classList.add('active');
        }
    });
}

// 防抖函数
function debounce(func, wait) {
    let timeout;
    return function executedFunction(...args) {
        const later = () => {
            clearTimeout(timeout);
            func(...args);
        };
        clearTimeout(timeout);
        timeout = setTimeout(later, wait);
    };
}

// 节流函数
function throttle(func, limit) {
    let inThrottle;
    return function(...args) {
        if (!inThrottle) {
            func.apply(this, args);
            inThrottle = true;
            setTimeout(() => inThrottle = false, limit);
        }
    };
}

// 导出全局函数
window.OMEGA = {
    api: apiRequest,
    toast: showToast,
    formatBytes,
    formatTimestamp,
    getHealthColor,
    layerColors: LAYER_COLORS,
    debounce,
    throttle
};