# Hermes Agent - Windows Desktop

☤ 基于 Tauri 2 + Vue 3 的 Hermes AI Agent 桌面客户端。

## 项目结构

```
hermes-windows/
├── src/                    # Vue3 前端
│   └── App.vue             # 主界面（对话 + 设置）
├── src-tauri/              # Rust 后端
│   ├── src/main.rs         # 核心逻辑
│   ├── Cargo.toml          # Rust 依赖
│   ├── tauri.conf.json     # Tauri 配置
│   └── bundle/             # 运行时捆绑目录
│       └── hermes-runtime/ # (生成) Python + Hermes Agent
├── scripts/                # 构建脚本
│   ├── bundle-runtime.py   # 打包 Python 运行环境
│   └── generate-icons.py   # 生成应用图标
├── .github/workflows/      # CI/CD
│   └── build-windows.yml
└── package.json
```

## 开发

```bash
# 安装前端依赖
npm install

# 安装 Tauri CLI
cargo install tauri-cli --version "^2.0" --locked

# 开发模式（热更新）
npm run tauri dev

# 打包
npm run tauri build --bundles nsis
```

## 构建 Windows 安装包

1. **准备运行环境**
   ```bash
   python scripts/bundle-runtime.py
   ```
   下载 embeddable Python + 安装依赖 + 复制 Hermes Agent。

2. **生成图标**
   ```bash
   pip install cairosvg Pillow
   python scripts/generate-icons.py
   ```

3. **打包**
   ```bash
   npm run tauri build --bundles nsis
   ```
   输出：`src-tauri/target/release/bundle/nsis/Hermes_*_x64-setup.exe`

## CI 构建

推送到 main 分支或打 v* 标签自动触发 GitHub Actions 构建：
- Windows: 生成 NSIS 安装包
- macOS: 验证编译

## 许可证系统

- 客户提供机器码 → 验证服务器返回激活码
- 200 元/月
- 支持离线验证（本地 HMAC 校验）
- 验证服务器：`hermes-saas/license_server.py`

## 架构

```
Tauri (Rust) → Hermes Agent (Python 子进程) → LLM API (MiniMax)
```

Windows 环境下，Python 3.11 embeddable + Hermes Agent 源码打包在安装包内。
