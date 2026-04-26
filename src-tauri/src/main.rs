// Prevents additional console window on Windows in release
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::OnceLock;
use tauri::Emitter;
use tauri::Manager;
use futures_util::StreamExt;
#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;

// 缓存 Python 路径，避免每次对话都重新检测
static CACHED_PYTHON: OnceLock<Mutex<Option<PathBuf>>> = OnceLock::new();
static CACHED_AGENT: OnceLock<Mutex<Option<PathBuf>>> = OnceLock::new();

fn get_python_cache() -> &'static Mutex<Option<PathBuf>> {
    CACHED_PYTHON.get_or_init(|| Mutex::new(None))
}

fn get_agent_cache() -> &'static Mutex<Option<PathBuf>> {
    CACHED_AGENT.get_or_init(|| Mutex::new(None))
}

// ============ Types ============

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct LicenseInfo {
    pub activated: bool,
    pub machine_code: String,
    pub expiry_date: Option<String>,
    pub days_left: i64,
    pub license_key: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ActivationResult {
    pub success: bool,
    pub message: String,
    pub expiry_date: Option<String>,
    pub license_key: String,
}

#[derive(Debug, Serialize)]
pub struct ChatResult {
    pub response: String,
    pub session_id: String,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct AppConfig {
    pub api_key: String,
    pub api_base: String,
    pub model: String,
    pub feishu_app_id: String,
    pub feishu_app_secret: String,
    pub feishu_chat_id: String,
}

// 南京验证服务器响应格式
#[derive(Debug, Deserialize)]
struct NanjingVerify {
    valid: bool,
    #[serde(default)]
    expiry_date: Option<String>,
    #[serde(default)]
    days_left: Option<i64>,
    #[allow(dead_code)]
    #[serde(default)]
    message: Option<String>,
}

#[derive(Debug, Deserialize)]
struct NanjingActivate {
    success: bool,
    #[allow(dead_code)]
    message: String,
    #[serde(default)]
    expiry_date: Option<String>,
}

// ============ Constants ============

const ACTIVATION_SALT: &[u8] = b"HermesAI_v1_2025";
const LICENSE_FILE: &str = "license.dat";
const DEFAULT_API_BASE: &str = "https://api.minimaxi.com/v1";
const LICENSE_SERVER: &str = "http://175.27.242.158:5000";

// 内置 MiniMax API Key（发布前确认额度充足）
const BUILTIN_API_KEY: &str = "sk-cp-_2yFksEQQQrzpyKNpNsPD7fiPiKbsOXJDLTfOwQdWDLZQro_iuG_UUFbrQOn9-g_WJPQtpf-MCx02bv89LYyhy6pI40TjrelWji--aLVNTN6fePCY64Udi0"; // MiniMax-M2.7-highspeed

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ApiMessage {
    pub role: String,
    pub content: String,
}

// ============ State ============

pub struct AppState {
    pub hermes_ready: Mutex<bool>,
    pub chat_history: Mutex<Vec<ApiMessage>>,
}

// ============ Helpers ============

/// 将进程输出字节解码为 UTF-8 String
/// 优先按 UTF-8 解码，失败时回退到 GBK（中文 Windows 常见编码）
fn decode_output(bytes: &[u8]) -> String {
    // 先尝试 UTF-8
    if let Ok(s) = std::str::from_utf8(bytes) {
        return s.to_string();
    }
    // 回退到 GBK 解码
    let (cow, _encoding, _had_errors) = encoding_rs::GBK.decode(bytes);
    cow.to_string()
}

/// 创建静默命令（Windows 下加 CREATE_NO_WINDOW）
fn silent_cmd(program: &str) -> std::process::Command {
    let mut cmd = std::process::Command::new(program);
    #[cfg(target_os = "windows")]
    cmd.creation_flags(0x08000000);
    cmd
}

/// 过滤模型的思考过程，只保留最终回答
/// MiniMax M2.7 在 <think>...</think> 标签中输出思考过程
fn strip_thinking(text: &str) -> String {
    let mut result = String::new();
    let mut in_think = false;

    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("<think>") {
            in_think = true;
            // 如果 <think> 后还有其他内容在同一行，保留
            let after = trimmed.trim_start_matches("<think>").trim();
            if !after.is_empty() && !after.starts_with("<think>") && !after.starts_with("</think>") {
                result.push_str(line);
                result.push('\n');
            }
            continue;
        }
        if trimmed.contains("</think>") {
            in_think = false;
            // 如果 </think> 后还有其他内容，保留
            let after = trimmed.split("</think>").nth(1).unwrap_or("").trim();
            if !after.is_empty() {
                result.push_str(after);
                result.push('\n');
            }
            continue;
        }
        if in_think {
            continue;
        }
        result.push_str(line);
        result.push('\n');
    }

    let result = result.trim().to_string();
    if result.is_empty() { text.to_string() } else { result }
}

/// 持久化 Python 路径，下次启动 APP 免去注册表查询
fn save_python_path(path: &Path) {
    if let Ok(data_dir) = get_data_dir() {
        let _ = std::fs::write(data_dir.join(".python_path"), path.to_string_lossy().as_ref());
    }
}

/// 读取持久化的 Python 路径缓存
fn load_cached_python_path() -> Option<PathBuf> {
    let data_dir = get_data_dir().ok()?;
    let cache_file = data_dir.join(".python_path");
    if !cache_file.exists() {
        return None;
    }
    let content = std::fs::read_to_string(cache_file).ok()?;
    let path = PathBuf::from(content.trim());
    if path.exists() {
        Some(path)
    } else {
        // 缓存的路径已不存在，清理
        let _ = std::fs::remove_file(data_dir.join(".python_path"));
        None
    }
}

/// 找到 Python 路径后统一处理：持久化 + 内存缓存 + 返回
fn report_python(path: PathBuf) -> Result<PathBuf, String> {
    save_python_path(&path);
    *get_python_cache().lock().unwrap() = Some(path.clone());
    Ok(path)
}

fn get_machine_id() -> Result<String, String> {
    let uid = machine_uid::get().map_err(|e| format!("获取机器码失败: {}", e))?;
    let mut hasher = Sha256::new();
    hasher.update(uid.as_bytes());
    hasher.update(ACTIVATION_SALT);
    Ok(hex::encode(hasher.finalize()))
}

fn checksum_from_bytes(bytes: &[u8]) -> String {
    hex::encode(&bytes[..4])
}

fn generate_activate_code(machine_id: &str, expiry_str: &str) -> String {
    let short_id = &machine_id[..8];
    let mut hasher = Sha256::new();
    hasher.update(machine_id.as_bytes());
    hasher.update(expiry_str.as_bytes());
    hasher.update(ACTIVATION_SALT);
    let result = hasher.finalize();
    let checksum = checksum_from_bytes(&result);
    let date_part = expiry_str.replace("-", "");
    format!("{}-{}{}", short_id, date_part, checksum)
}

fn parse_activation_code(code: &str, machine_id: &str) -> Result<String, String> {
    let parts: Vec<&str> = code.split('-').collect();
    if parts.len() != 3 {
        return Err("激活码格式无效".to_string());
    }
    let stored_id = parts[0];
    let date_part = parts[1];
    let stored_checksum = parts[2];
    let full_machine_id = &machine_id[..8];
    if stored_id != full_machine_id {
        return Err("激活码与当前机器不匹配".to_string());
    }
    let expiry_str = format!("{}-{}-{}", &date_part[..4], &date_part[4..6], &date_part[6..8]);
    let mut hasher = Sha256::new();
    hasher.update(machine_id.as_bytes());
    hasher.update(expiry_str.as_bytes());
    hasher.update(ACTIVATION_SALT);
    let result = hasher.finalize();
    let expected_checksum = checksum_from_bytes(&result);
    if stored_checksum != expected_checksum {
        return Err("激活码校验失败".to_string());
    }
    Ok(expiry_str)
}

// ============ File Paths ============

fn get_data_dir() -> Result<PathBuf, String> {
    let data_dir = dirs::data_local_dir()
        .ok_or("无法获取数据目录")?
        .join("HermesAI");
    std::fs::create_dir_all(&data_dir).map_err(|e| format!("创建数据目录失败: {}", e))?;
    Ok(data_dir)
}

fn get_config_path() -> Result<PathBuf, String> {
    Ok(get_data_dir()?.join("config.json"))
}

// ============ Config ============

impl AppConfig {
    fn load() -> Result<Self, String> {
        let path = get_config_path()?;
        if !path.exists() {
            return Ok(AppConfig {
                api_key: String::new(),
                api_base: DEFAULT_API_BASE.to_string(),
                model: "MiniMax-M2.7-highspeed".to_string(),
                ..Default::default()
            });
        }
        let content = std::fs::read_to_string(&path).map_err(|e| format!("读取配置失败: {}", e))?;
        serde_json::from_str(&content).map_err(|e| format!("解析配置失败: {}", e))
    }

    fn save(&self) -> Result<(), String> {
        let path = get_config_path()?;
        let content = serde_json::to_string_pretty(self).map_err(|e| format!("序列化失败: {}", e))?;
        std::fs::write(&path, content).map_err(|e| format!("写入配置失败: {}", e))
    }

    fn effective_api_key(&self) -> String {
        if !self.api_key.is_empty() {
            self.api_key.clone()
        } else if !BUILTIN_API_KEY.is_empty() {
            BUILTIN_API_KEY.to_string()
        } else {
            String::new()
        }
    }
}

// ============ Hermes Agent Subprocess ============

/// 创建一个配置好的 Python 进程 Command。
/// 如果找到的是 `py.exe`（Python Launcher），会自动添加 `-3` 参数。
fn new_python_cmd(python: &Path) -> std::process::Command {
    let mut cmd = std::process::Command::new(python);
    #[cfg(target_os = "windows")]
    {
        if let Some(name) = python.file_name().and_then(|n| n.to_str()) {
            if name.eq_ignore_ascii_case("py.exe") || name.eq_ignore_ascii_case("py") {
                cmd.arg("-3");
            }
        }
    }
    cmd
}

fn find_hermes_python() -> Result<PathBuf, String> {
    // 先检查进程级缓存
    if let Some(cached) = get_python_cache().lock().unwrap().as_ref() {
        if cached.exists() {
            return Ok(cached.clone());
        }
    }

    // 再检查持久化缓存（免去注册表查询，跨会话）
    if let Some(persisted) = load_cached_python_path() {
        *get_python_cache().lock().unwrap() = Some(persisted.clone());
        return Ok(persisted);
    }

    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|p| p.to_path_buf()))
        .unwrap_or_default();

    // 1. 和 exe 同目录的 python/python.exe
    let bundled = exe_dir.join("python").join("python.exe");
    if bundled.exists() {
        return report_python(bundled);
    }

    // 2. 环境变量 HERMES_PYTHON
    if let Ok(env_py) = std::env::var("HERMES_PYTHON") {
        let p = PathBuf::from(env_py);
        if p.exists() {
            return report_python(p);
        }
    }

    #[cfg(target_os = "windows")]
    {
        // 辅助: 测试 Python 确实能启动
        fn python_works(path: &std::path::Path) -> bool {
            let mut cmd = std::process::Command::new(path);
            cmd.arg("--version")
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null());
            #[cfg(target_os = "windows")]
            cmd.creation_flags(0x08000000);
            cmd.output()
                .map(|o| o.status.success())
                .unwrap_or(false)
        }

        // 从注册表查找 Python（最可靠的方法，py.exe 内部也这么找）
        {
            let mut reg_cmd = std::process::Command::new("reg");
            reg_cmd.args(["query", "HKLM\\Software\\Python\\PythonCore", "/s", "/v", "InstallPath"]);
            #[cfg(target_os = "windows")]
            reg_cmd.creation_flags(0x08000000);
            if let Ok(output) = reg_cmd.stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::null()).output() {
                if output.status.success() {
                    for line in decode_output(&output.stdout).lines() {
                        let line = line.trim();
                        // 查找 "InstallPath    REG_SZ    C:\..."
                        if let Some(idx) = line.rfind("REG_SZ") {
                            let dir = line[idx + 6..].trim().trim_matches('"');
                            if !dir.is_empty() {
                                let py_exe = PathBuf::from(dir).join("python.exe");
                                if py_exe.exists() && python_works(&py_exe) {
                                    return report_python(py_exe);
                                }
                            }
                        }
                    }
                }
            }
            // 也查 HKCU（当前用户安装）
            let mut reg_cmd2 = std::process::Command::new("reg");
            reg_cmd2.args(["query", "HKCU\\Software\\Python\\PythonCore", "/s", "/v", "InstallPath"]);
            #[cfg(target_os = "windows")]
            reg_cmd2.creation_flags(0x08000000);
            if let Ok(output) = reg_cmd2.stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::null()).output() {
                if output.status.success() {
                    for line in decode_output(&output.stdout).lines() {
                        let line = line.trim();
                        if let Some(idx) = line.rfind("REG_SZ") {
                            let dir = line[idx + 6..].trim().trim_matches('"');
                            if !dir.is_empty() {
                                let py_exe = PathBuf::from(dir).join("python.exe");
                                if py_exe.exists() && python_works(&py_exe) {
                                    return report_python(py_exe);
                                }
                            }
                        }
                    }
                }
            }
        }

        // 3. Program Files 系统级安装
        let program_paths = [
            "C:\\Program Files\\Python313\\python.exe",
            "C:\\Program Files\\Python312\\python.exe",
            "C:\\Program Files\\Python311\\python.exe",
            "C:\\Program Files\\Python310\\python.exe",
            "C:\\Python313\\python.exe",
            "C:\\Python312\\python.exe",
            "C:\\Python311\\python.exe",
            "C:\\Python310\\python.exe",
        ];
        for p in &program_paths {
            let pb = PathBuf::from(p);
            if pb.exists() && python_works(&pb) {
                return report_python(pb);
            }
        }

        // 4. AppData Python 安装
        if let Ok(local_app_data) = std::env::var("LOCALAPPDATA") {
            let py_base = PathBuf::from(local_app_data).join("Programs\\Python");
            if py_base.exists() {
                if let Ok(entries) = std::fs::read_dir(&py_base) {
                    for entry in entries.flatten() {
                        let python_exe = entry.path().join("python.exe");
                        if python_exe.exists() && python_works(&python_exe) {
                            return report_python(python_exe);
                        }
                    }
                }
            }
        }

        // 5. Anaconda / Miniconda
        if let Ok(user_profile) = std::env::var("USERPROFILE") {
            for conda_dir in &["anaconda3", "miniconda3", "Anaconda3", "Miniconda3"] {
                let pb = PathBuf::from(&user_profile).join(conda_dir).join("python.exe");
                if pb.exists() && python_works(&pb) {
                    return report_python(pb);
                }
            }
        }

        // 6. where.exe python.exe（跳过 WindowsApps 存根）
        {
            let mut where_cmd = std::process::Command::new("where.exe");
            where_cmd.arg("python.exe")
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::null());
            #[cfg(target_os = "windows")]
            where_cmd.creation_flags(0x08000000);
            if let Ok(output) = where_cmd.output() {
                if output.status.success() {
                    for line in decode_output(&output.stdout).lines() {
                        let path = line.trim().to_string();
                        if path.is_empty() || path.contains("WindowsApps") {
                            continue;
                        }
                        let pb = PathBuf::from(&path);
                        if pb.exists() && python_works(&pb) {
                            return report_python(pb);
                        }
                    }
                }
            }
        }

        // 7. py.exe (Python Launcher) — 最后备选
        //    先用它解析出真实 python.exe 路径
        if python_works(Path::new("py.exe")) {
            let mut resolve = std::process::Command::new("py.exe");
            resolve.args(["-3", "-c", "import sys; print(sys.executable, end='')"]);
            #[cfg(target_os = "windows")]
            resolve.creation_flags(0x08000000);
            if let Ok(out) = resolve.output() {
                if out.status.success() {
                    let real_path = decode_output(&out.stdout).trim().to_string();
                    if !real_path.is_empty() {
                        let pb = PathBuf::from(&real_path);
                        if pb.exists() {
                            return report_python(pb);
                        }
                    }
                }
            }
            // 真的找不到，返回 py.exe（会弹黑窗但至少能用）
            return Ok(PathBuf::from("py.exe"));
        }
    }

    // 最后尝试 PATH 搜索
    let system = PathBuf::from(
        if cfg!(target_os = "windows") { "python.exe" } else { "python3" }
    );
    if silent_cmd(&system.to_string_lossy())
        .arg("--version")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
    {
        return Ok(system);
    }

    Err("未找到 Python，请先安装 Python 3.10+".to_string())
}

fn find_hermes_agent() -> Result<PathBuf, String> {
    // 先检查缓存
    if let Some(cached) = get_agent_cache().lock().unwrap().as_ref() {
        if cached.exists() {
            return Ok(cached.clone());
        }
    }

    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|p| p.to_path_buf()))
        .unwrap_or_default();

    // 1. 和 exe 同目录的 hermes-agent/
    let bundled = exe_dir.join("hermes-agent").join("hermes");
    if bundled.exists() {
        *get_agent_cache().lock().unwrap() = Some(bundled.clone());
        return Ok(bundled);
    }

    // 2. 应用数据目录（setup_hermes_environment 下载的位置）
    if let Ok(data_dir) = get_data_dir() {
        let data_path = data_dir.join("hermes-agent").join("hermes");
        if data_path.exists() {
            *get_agent_cache().lock().unwrap() = Some(data_path.clone());
            return Ok(data_path);
        }
    }

    // 3. Mac 开发环境
    let dev_path = PathBuf::from("/Users/laomashitu/hermes-agent/hermes");
    if dev_path.exists() {
        *get_agent_cache().lock().unwrap() = Some(dev_path.clone());
        return Ok(dev_path);
    }

    // 4. 环境变量
    if let Ok(env_path) = std::env::var("HERMES_AGENT") {
        let p = PathBuf::from(env_path);
        if p.exists() {
            *get_agent_cache().lock().unwrap() = Some(p.clone());
            return Ok(p);
        }
    }

    Err("未找到 Hermes Agent，请先安装运行环境".to_string())
}

fn ensure_hermes_deps(python: &Path, agent_dir: &Path) -> Result<(), String> {
    // 缓存标记：依赖验证通过后写一个标记文件，下次跳过检查
    if let Ok(data_dir) = get_data_dir() {
        let marker = data_dir.join(".deps_ok");
        if marker.exists() {
            return Ok(());
        }
    }

    // 检查核心 Python 依赖是否已安装（静默执行，不显示黑窗）
    let deps_ok = {
        let mut cmd = new_python_cmd(python);
        cmd.arg("-c").arg("import yaml, prompt_toolkit, openai, rich")
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null());
        #[cfg(target_os = "windows")]
        cmd.creation_flags(0x08000000);
        cmd.output().map_err(|e| format!("检查依赖失败: {}", e))?
            .status.success()
    };

    if deps_ok {
        // 写入标记文件，后续对话跳过依赖检查
        if let Ok(data_dir) = get_data_dir() {
            let _ = std::fs::write(data_dir.join(".deps_ok"), b"1");
        }
        return Ok(());
    }

    // 缺失依赖，自动静默安装
    let mut install = new_python_cmd(python);
    install.arg("-m").arg("pip").arg("install")
        .arg(agent_dir.to_string_lossy().to_string())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    #[cfg(target_os = "windows")]
    install.creation_flags(0x08000000);
    if install.output().map_err(|e| format!("自动安装依赖失败: {}", e))?.status.success() {
        if let Ok(data_dir) = get_data_dir() {
            let _ = std::fs::write(data_dir.join(".deps_ok"), b"1");
        }
        return Ok(());
    }

    // 备用：逐个安装核心依赖
    let mut fallback = new_python_cmd(python);
    fallback.arg("-m").arg("pip").arg("install")
        .arg("openai").arg("anthropic").arg("httpx[socks]")
        .arg("rich").arg("fire").arg("tenacity")
        .arg("pyyaml").arg("requests").arg("jinja2")
        .arg("pydantic").arg("prompt_toolkit").arg("python-dotenv")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    #[cfg(target_os = "windows")]
    fallback.creation_flags(0x08000000);
    if !fallback.output().map_err(|e| format!("安装核心依赖失败: {}", e))?.status.success() {
        return Err("自动安装 Python 依赖失败，请尝试在设置页面重新安装运行环境".to_string());
    }

    if let Ok(data_dir) = get_data_dir() {
        let _ = std::fs::write(data_dir.join(".deps_ok"), b"1");
    }
    Ok(())
}

fn run_hermes_chat(prompt: &str, session_id: &str) -> Result<(String, String), String> {
    let python = find_hermes_python()?;
    let hermes = find_hermes_agent()?;

    // 自动检查并安装 Python 依赖（如果缺失）
    if let Some(agent_dir) = hermes.parent() {
        ensure_hermes_deps(&python, agent_dir)?;
    }

    let config = AppConfig::load()?;
    let api_key = config.effective_api_key();

    if api_key.is_empty() {
        return Err("请先在设置中配置 API Key，或联系作者获取内置 Key".to_string());
    }

    let mut cmd = new_python_cmd(&python);

    cmd.env("MINIMAX_API_KEY", &api_key)
        .env("MINIMAX_CN_API_KEY", &api_key)
        .env("HERMES_Q", prompt)
        .env("PYTHONUTF8", "1")
        .env("PYTHONIOENCODING", "utf-8")
        .env("PYTHONUNBUFFERED", "1");
    // 使用 -c 方式直接调用 cli.main()，完全规避 Windows 上无扩展名脚本执行问题
    // 同时强制设置 stdout 编码为 utf-8，确保 GBK 系统下输出不乱码
    if let Some(agent_dir) = hermes.parent() {
        let agent_dir_escaped = agent_dir.to_string_lossy().replace('\\', "\\\\").replace('\'', "\\'");
        let resume_arg = if !session_id.is_empty() {
            format!(", resume='{}'", session_id.replace('\'', "\\'"))
        } else {
            String::new()
        };
        let python_code = format!(
            "import sys,os; sys.stdout=__import__('io').TextIOWrapper(sys.stdout.buffer,encoding='utf-8',errors='replace'); sys.path.insert(0, '{}'); from cli import main; main(query=os.environ.get('HERMES_Q',''), quiet=False{})",
            agent_dir_escaped, resume_arg
        );
        cmd.env("PYTHONPATH", agent_dir.to_string_lossy().to_string());
        cmd.arg("-c").arg(&python_code);
    } else {
        cmd.arg(&hermes)
            .arg("chat")
            .arg("-q")
            .arg(prompt)
            .arg("-Q");
        if !session_id.is_empty() {
            cmd.args(["--resume", session_id]);
        }
    }

    #[cfg(target_os = "windows")]
    {
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }

    let output = cmd.output().map_err(|e| format!("启动 Hermes Agent 失败: {}", e))?;

    if !output.status.success() {
        let stderr = decode_output(&output.stderr);
        let stdout = decode_output(&output.stdout);
        let exit_code = output.status.code().unwrap_or(-1);
        return Err(format!(
            "Hermes Agent 错误 (exit:{})\n----stderr----\n{}\n----stdout----\n{}",
            exit_code, stderr, stdout
        ));
    }

    let raw = decode_output(&output.stdout);
    let (response, new_session) = parse_hermes_output(&raw, session_id);

    Ok((response, new_session))
}

fn parse_hermes_output(raw: &str, old_session: &str) -> (String, String) {
    let mut lines: Vec<&str> = Vec::new();
    let mut new_session = old_session.to_string();

    for line in raw.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("session_id:") {
            new_session = trimmed.trim_start_matches("session_id:").trim().to_string();
            continue;
        }
        // 过滤掉 Hermes 的工具日志
        if trimmed.is_empty()
            || trimmed.starts_with("╭─")
            || trimmed.starts_with("╰─")
            || trimmed.starts_with("│")
            || trimmed.starts_with("Session:")
            || trimmed.starts_with("Duration:")
            || trimmed.starts_with("Messages:")
            || trimmed.contains("⚠️")
            || trimmed.contains("🔍")
            || trimmed.contains("💻")
            || trimmed.starts_with("──")
        {
            continue;
        }
        lines.push(trimmed);
    }

    (lines.join("\n"), new_session)
}

// ============ 流式读取（逐行思考过程） ============

/// 逐行读取 Python 进程 stdout，实时发送到前端
fn run_chat_stream_impl(
    python: &Path,
    hermes: &Path,
    prompt: &str,
    session_id: &str,
    api_key: &str,
    app_handle: &tauri::AppHandle,
) -> Result<(String, String), String> {
    let mut cmd = new_python_cmd(python);

    cmd.env("MINIMAX_API_KEY", api_key)
        .env("MINIMAX_CN_API_KEY", api_key)
        .env("HERMES_Q", prompt)
        .env("PYTHONUTF8", "1")
        .env("PYTHONIOENCODING", "utf-8")
        .env("PYTHONUNBUFFERED", "1");

    if let Some(agent_dir) = hermes.parent() {
        let agent_dir_escaped = agent_dir.to_string_lossy().replace('\\', "\\\\").replace('\'', "\\'");
        let resume_arg = if !session_id.is_empty() {
            format!(", resume='{}'", session_id.replace('\'', "\\'"))
        } else {
            String::new()
        };
        let python_code = format!(
            "import sys,os; sys.stdout=__import__('io').TextIOWrapper(sys.stdout.buffer,encoding='utf-8',errors='replace'); sys.path.insert(0, '{}'); from cli import main; main(query=os.environ.get('HERMES_Q',''), quiet=False{})",
            agent_dir_escaped, resume_arg
        );
        cmd.env("PYTHONPATH", agent_dir.to_string_lossy().to_string());
        cmd.arg("-c").arg(&python_code);
    } else {
        cmd.arg(hermes)
            .arg("chat")
            .arg("-q")
            .arg(prompt)
            .arg("-Q");
        if !session_id.is_empty() {
            cmd.args(["--resume", session_id]);
        }
    }

    #[cfg(target_os = "windows")]
    cmd.creation_flags(0x08000000);

    let mut child = cmd
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| format!("启动 Hermes Agent 失败: {}", e))?;

    let stdout = child.stdout.take().expect("无法获取 stdout");
    let stderr = child.stderr.take().expect("无法获取 stderr");

    // 后台线程读取 stderr，防止管道阻塞
    let stderr_thread = std::thread::spawn(move || {
        let mut buf = Vec::new();
        let mut reader = BufReader::new(stderr);
        reader.read_to_end(&mut buf).ok();
        buf
    });

    // 主线程逐行读取 stdout，实时转发
    let mut reader = BufReader::new(stdout);
    let mut full_output = String::new();
    let mut line_buf: Vec<u8> = Vec::new();

    loop {
        line_buf.clear();
        match reader.read_until(b'\n', &mut line_buf) {
            Ok(0) => break,
            Ok(_) => {
                let decoded = decode_output(&line_buf);
                let trimmed = decoded.trim();
                full_output.push_str(trimmed);
                full_output.push('\n');

                // 忽略空行和纯装饰线（保留 emoji 状态行）
                if !trimmed.is_empty() {
                    let _ = app_handle.emit("chat-stream-line", trimmed);
                }
            }
            Err(e) => {
                eprintln!("读取 stdout 流错误: {}", e);
                break;
            }
        }
    }

    drop(reader);

    let status = child.wait().map_err(|e| format!("等待进程结束失败: {}", e))?;
    let stderr_bytes = stderr_thread.join().unwrap_or_default();
    let stderr_text = decode_output(&stderr_bytes);

    if !status.success() {
        return Err(format!(
            "Hermes Agent 错误 (exit:{:?})\n{}",
            status.code(),
            stderr_text
        ));
    }

    let (response, new_session) = parse_hermes_output(&full_output, session_id);
    Ok((response, new_session))
}

#[tauri::command]
async fn chat_stream(prompt: String, session_id: String, app_handle: tauri::AppHandle) -> Result<ChatResult, String> {
    // 验证许可证
    let license_path = get_data_dir()?.join(LICENSE_FILE);
    let saved_key = if license_path.exists() {
        std::fs::read_to_string(&license_path).unwrap_or_default()
    } else {
        String::new()
    };
    if saved_key.is_empty() {
        return Err("软件未激活，请先激活许可证".to_string());
    }

    let result = tokio::task::spawn_blocking(move || {
        let python = find_hermes_python()?;
        let hermes = find_hermes_agent()?;
        if let Some(agent_dir) = hermes.parent() {
            ensure_hermes_deps(&python, agent_dir)?;
        }
        let config = AppConfig::load()?;
        let api_key = config.effective_api_key();
        if api_key.is_empty() {
            return Err("请先在设置中配置 API Key，或联系作者获取内置 Key".to_string());
        }
        run_chat_stream_impl(&python, &hermes, &prompt, &session_id, &api_key, &app_handle)
    })
    .await
    .map_err(|e| format!("内部线程错误: {}", e))?;

    let (response, new_session) = result?;
    Ok(ChatResult { response, session_id: new_session })
}

// ============ 直接 API 调用（跳过 Hermes Agent，速度最快） ============

/// 直接用 MiniMax 的 Anthropic 兼容 API，不经过 Hermes Agent
#[tauri::command]
async fn chat_direct(prompt: String, app_handle: tauri::AppHandle) -> Result<ChatResult, String> {
    // 验证许可证
    let license_path = get_data_dir()?.join(LICENSE_FILE);
    let saved_key = if license_path.exists() {
        std::fs::read_to_string(&license_path).unwrap_or_default()
    } else {
        String::new()
    };
    if saved_key.is_empty() {
        return Err("软件未激活，请先激活许可证".to_string());
    }

    let config = AppConfig::load()?;

    // 使用内置 Key 时忽略 config.json 中可能残留的错误配置
    let use_builtin = config.api_key.is_empty() || config.api_key == BUILTIN_API_KEY;
    let (api_key, model, api_base) = if use_builtin {
        (BUILTIN_API_KEY.to_string(), "MiniMax-M2.7-highspeed".to_string(), DEFAULT_API_BASE.to_string())
    } else {
        let key = config.api_key.clone();
        if key.is_empty() {
            return Err("请先在设置中配置 API Key，或联系作者获取内置 Key".to_string());
        }
        (key, config.model.clone(), config.api_base.clone())
    };

    // 获取对话历史
    let history = app_handle.state::<AppState>().chat_history.lock().unwrap().clone();

    // 构建消息列表（含历史）
    let mut messages = history;
    messages.push(ApiMessage {
        role: "user".to_string(),
        content: prompt.clone(),
    });

    // MiniMax 使用 OpenAI 兼容 API
    let base = api_base.trim_end_matches('/');
    let url = format!("{}/chat/completions", base);

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(120))
        .build()
        .map_err(|e| format!("HTTP 客户端错误: {}", e))?;

    let request_body = serde_json::json!({
        "model": model,
        "max_tokens": 4096,
        "messages": messages,
        "enable_search": true,
    });

    let response = client
        .post(&url)
        .header("Authorization", format!("Bearer {}", api_key))
        .header("Content-Type", "application/json")
        .json(&request_body)
        .send()
        .await
        .map_err(|e| format!("API 请求失败: {}", e))?;

    if !response.status().is_success() {
        let status = response.status();
        let err_text = response.text().await.unwrap_or_default();
        return Err(format!("API 错误 ({}): {}", status, err_text));
    }

    // 解析响应体
    let body: serde_json::Value = response
        .json()
        .await
        .map_err(|e| format!("解析 API 响应失败: {}", e))?;

    // OpenAI 格式: body["choices"][0]["message"]["content"]
    let raw_text = body["choices"]
        .as_array()
        .and_then(|arr| arr.first())
        .and_then(|choice| choice["message"]["content"].as_str())
        .unwrap_or("")
        .to_string();

    if raw_text.is_empty() {
        return Err("API 返回空响应".to_string());
    }

    // 分离 thinking 和最终回答，过滤掉思考过程
    let text = strip_thinking(&raw_text);

    // 只发射最终回答到流式输出（不含思考过程）
    let mut emitted = String::new();
    for ch in text.chars() {
        emitted.push(ch);
        // 按句或按行发射
        if ch == '\n' || ch == '。' || ch == '！' || ch == '？' || emitted.len() >= 50 {
            let _ = app_handle.emit("chat-stream-line", &emitted);
            emitted.clear();
        }
    }
    if !emitted.is_empty() {
        let _ = app_handle.emit("chat-stream-line", &emitted);
    }

    // 更新对话历史
    let state = app_handle.state::<AppState>();
    let mut history = state.chat_history.lock().unwrap();
    history.push(ApiMessage {
        role: "user".to_string(),
        content: prompt,
    });
    history.push(ApiMessage {
        role: "assistant".to_string(),
        content: text.clone(),
    });
    // 只保留最近 20 轮对话
    while history.len() > 40 {
        history.remove(0);
    }
    drop(history);

    Ok(ChatResult {
        response: text,
        session_id: String::new(),
    })
}

// ============ License Server HTTP Client ============

async fn verify_with_server(machine_code: &str) -> Result<LicenseInfo, String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| format!("HTTP client error: {}", e))?;

    let resp = client
        .post(format!("{}/verify", LICENSE_SERVER))
        .json(&serde_json::json!({
            "machine_code": machine_code,
        }))
        .send()
        .await;

    match resp {
        Ok(r) => {
            let data: NanjingVerify = r.json().await.map_err(|_| "解析服务器响应失败")?;
            let saved_key_path = get_data_dir()?.join(LICENSE_FILE);
            let saved_key = if saved_key_path.exists() {
                std::fs::read_to_string(&saved_key_path).unwrap_or_default()
            } else {
                String::new()
            };
            Ok(LicenseInfo {
                activated: data.valid,
                machine_code: machine_code.to_string(),
                expiry_date: data.expiry_date,
                days_left: data.days_left.unwrap_or(0),
                license_key: saved_key,
            })
        }
        Err(e) => Err(format!("无法连接验证服务器: {}", e)),
    }
}

// ============ Tauri Commands ============

#[tauri::command]
async fn get_machine_code() -> Result<String, String> {
    get_machine_id()
}

#[tauri::command]
async fn get_license_status() -> Result<LicenseInfo, String> {
    let machine_id = get_machine_id()?;
    let license_path = get_data_dir()?.join(LICENSE_FILE);

    // 读取本地缓存的激活码
    let saved_key = if license_path.exists() {
        std::fs::read_to_string(&license_path).unwrap_or_default()
    } else {
        String::new()
    };

    if saved_key.is_empty() {
        return Ok(LicenseInfo {
            activated: false,
            machine_code: machine_id,
            expiry_date: None,
            days_left: 0,
            license_key: String::new(),
        });
    }

    // 向服务器验证（仅需机器码，Nanjing server 会自动匹配）
    verify_with_server(&machine_id).await
}

#[tauri::command]
async fn activate_license(activation_code: String) -> Result<ActivationResult, String> {
    let machine_id = get_machine_id()?;
    let code = activation_code.trim().to_uppercase();

    // 先尝试本地离线验证（兼容无网络环境）
    if let Ok(expiry) = parse_activation_code(&code, &machine_id) {
        let expires = chrono::NaiveDate::parse_from_str(&expiry, "%Y-%m-%d")
            .map_err(|e| format!("日期解析失败: {}", e))?;
        let days_left = (expires - chrono::Local::now().date_naive()).num_days();
        std::fs::write(get_data_dir()?.join(LICENSE_FILE), &code)
            .map_err(|e| format!("保存许可证失败: {}", e))?;
        return Ok(ActivationResult {
            success: true,
            message: format!("激活成功！剩余 {} 天，到期日: {}", days_left, expiry),
            expiry_date: Some(expiry),
            license_key: code,
        });
    };

    // 本地验证失败，尝试 Nanjing 服务器验证
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| format!("HTTP error: {}", e))?;

    let resp = client
        .post(format!("{}/activate", LICENSE_SERVER))
        .json(&serde_json::json!({
            "machine_code": machine_id,
            "activation_key": code,
        }))
        .send()
        .await
        .map_err(|e| format!("无法连接验证服务器: {}", e))?;

    let data: NanjingActivate = resp.json().await.map_err(|_| "解析服务器响应失败")?;

    if !data.success {
        return Err(format!("激活失败：{}。请联系作者 13213181166", data.message));
    }

    // 保存激活码
    std::fs::write(get_data_dir()?.join(LICENSE_FILE), &code)
        .map_err(|e| format!("保存许可证失败: {}", e))?;

    let expiry_date = data.expiry_date.unwrap_or_default();
    let display_date = if expiry_date.len() >= 10 { &expiry_date[..10] } else { &expiry_date };

    Ok(ActivationResult {
        success: true,
        message: format!("激活成功！到期日: {}", display_date),
        expiry_date: Some(expiry_date),
        license_key: code,
    })
}

#[tauri::command]
async fn deactivate_license() -> Result<ActivationResult, String> {
    let path = get_data_dir()?.join(LICENSE_FILE);
    if path.exists() {
        std::fs::remove_file(&path).map_err(|e| format!("删除许可证失败: {}", e))?;
    }
    Ok(ActivationResult {
        success: true,
        message: "许可证已注销".to_string(),
        expiry_date: None,
        license_key: String::new(),
    })
}

#[tauri::command]
async fn generate_activation_code(machine_code: String, months: u32) -> Result<String, String> {
    let today = chrono::Local::now().date_naive();
    let days = (months * 30) as i64;
    let expiry = today + chrono::Duration::days(days);
    let expiry_str = expiry.format("%Y-%m-%d").to_string();
    Ok(generate_activate_code(&machine_code, &expiry_str))
}

#[tauri::command]
async fn chat_completion(prompt: String, session_id: String) -> Result<ChatResult, String> {
    // 验证许可证
    let license_path = get_data_dir()?.join(LICENSE_FILE);
    let saved_key = if license_path.exists() {
        std::fs::read_to_string(&license_path).unwrap_or_default()
    } else {
        String::new()
    };
    if saved_key.is_empty() {
        return Err("软件未激活，请先激活许可证".to_string());
    }

    // 调用 Hermes Agent
    let (response, new_session) = run_hermes_chat(&prompt, &session_id)?;
    Ok(ChatResult {
        response,
        session_id: new_session,
    })
}

#[tauri::command]
async fn save_api_config(api_key: String, api_base: String, model: String) -> Result<(), String> {
    let mut config = AppConfig::load()?;
    config.api_key = api_key;
    config.api_base = api_base;
    config.model = model;
    config.save()
}

#[tauri::command]
async fn get_api_config() -> Result<AppConfig, String> {
    let mut config = AppConfig::load()?;
    // 使用内置 Key 时不给前端返回真实 Key，防止泄露
    if config.api_key.is_empty() && !BUILTIN_API_KEY.is_empty() {
        config.api_key = String::new();
    }
    Ok(config)
}

#[tauri::command]
async fn save_feishu_config(app_id: String, app_secret: String, chat_id: String) -> Result<(), String> {
    let mut config = AppConfig::load()?;
    config.feishu_app_id = app_id;
    config.feishu_app_secret = app_secret;
    config.feishu_chat_id = chat_id;
    config.save()
}

#[tauri::command]
async fn test_feishu() -> Result<String, String> {
    let config = AppConfig::load()?;
    if config.feishu_app_id.is_empty() || config.feishu_chat_id.is_empty() {
        return Err("请先配置飞书 App ID 和群ID".to_string());
    }
    let token = get_feishu_token(&config.feishu_app_id, &config.feishu_app_secret).await?;
    send_feishu_message(&token, &config.feishu_chat_id, "这是一条来自 Hermes 的测试消息").await?;
    Ok("测试消息发送成功".to_string())
}

#[tauri::command]
async fn check_hermes_environment() -> Result<serde_json::Value, String> {
    let python = find_hermes_python();
    let agent = find_hermes_agent();

    let python_ok = python.is_ok();
    let agent_ok = agent.is_ok();

    // 检查 Node.js 是否可用（Web UI 需要）
    let node_version = check_node_installed();

    let mut status = serde_json::json!({
        "python_ok": python_ok,
        "agent_ok": agent_ok,
        "node_ok": node_version.is_some(),
        "node_version": node_version.unwrap_or_default(),
        "python_path": python.as_ref().map(|p| p.to_string_lossy().to_string()).unwrap_or_default(),
        "agent_path": agent.as_ref().map(|p| p.to_string_lossy().to_string()).unwrap_or_default(),
        "ready": python_ok && agent_ok,
    });

    if let Ok(ref p) = python {
        if let Ok(ref a) = agent {
            let test = new_python_cmd(p)
                .arg(a)
                .arg("--version")
                .output();
            if let Ok(out) = test {
                let version = decode_output(&out.stdout).trim().to_string();
                status["version"] = serde_json::Value::String(version);
            }
        }
    }

    Ok(status)
}

/// 检查 Node.js 是否安装（系统或本地捆绑）
fn check_node_installed() -> Option<String> {
    let node_name = if cfg!(target_os = "windows") { "node.exe" } else { "node" };
    let mut cmd = std::process::Command::new(node_name);
    cmd.arg("--version")
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null());
    #[cfg(target_os = "windows")]
    cmd.creation_flags(0x08000000);
    if let Ok(output) = cmd.output() {
        if output.status.success() {
            return Some(decode_output(&output.stdout).trim().to_string());
        }
    }
    // 检查本地捆绑
    if let Ok(data_dir) = get_data_dir() {
        let bundled = get_nodejs_dir(&data_dir).join(node_name);
        if bundled.exists() {
            let mut cmd = std::process::Command::new(&bundled);
            cmd.arg("--version")
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::null());
            #[cfg(target_os = "windows")]
            cmd.creation_flags(0x08000000);
            if let Ok(output) = cmd.output() {
                if output.status.success() {
                    return Some(format!("{} (本地)", decode_output(&output.stdout).trim()));
                }
            }
        }
    }
    None
}

// ============ Environment Setup ============

#[tauri::command]
async fn setup_hermes_environment(app_handle: tauri::AppHandle) -> Result<serde_json::Value, String> {
    let data_dir = get_data_dir()?;
    let extract_dir = data_dir.join("hermes-agent");
    let zip_path = data_dir.join("hermes-agent.zip");

    // Step 1: 获取 zip 文件
    // 安装后默认在 exe 同目录下，也检查 resource_dir
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|p| p.to_path_buf()))
        .unwrap_or_default();

    let resource_dir = app_handle.path()
        .resource_dir()
        .unwrap_or_default();

    let builtin_paths = [
        exe_dir.join("hermes-agent.zip"),
        resource_dir.join("hermes-agent.zip"),
    ];

    let bytes = if let Some(path) = builtin_paths.iter().find(|p| p.exists()) {
        std::fs::read(path).map_err(|e| format!("读取内置安装包失败: {}", e))?
    } else {
        // 从南京云服务器下载（便携版或开发环境）
        let download_url = "http://175.27.242.158:5000/download/hermes-agent.zip";
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(120))
            .build()
            .map_err(|e| format!("HTTP client error: {}", e))?;

        let response = client
            .get(download_url)
            .send()
            .await
            .map_err(|e| format!("下载 Hermes Agent 失败: {}", e))?;

        if !response.status().is_success() {
            return Err(format!("服务器返回错误: {}", response.status()));
        }

        response
            .bytes()
            .await
            .map_err(|e| format!("读取下载数据失败: {}", e))?
            .to_vec()
    };

    // 写入临时文件
    std::fs::write(&zip_path, &bytes).map_err(|e| format!("写入文件失败: {}", e))?;

    // Step 2: 解压
    let extract_result = {
        #[cfg(target_os = "windows")]
        {
            const CREATE_NO_WINDOW: u32 = 0x08000000;
            std::process::Command::new("powershell")
                .arg("-NoProfile")
                .arg("-Command")
                .arg(format!(
                    "Expand-Archive -Path '{}' -DestinationPath '{}' -Force",
                    zip_path.to_string_lossy(),
                    extract_dir.to_string_lossy()
                ))
                .creation_flags(CREATE_NO_WINDOW)
                .output()
                .map_err(|e| format!("解压失败: {}", e))?
        }
        #[cfg(not(target_os = "windows"))]
        {
            std::process::Command::new("unzip")
                .arg("-o")
                .arg(zip_path.to_string_lossy().to_string())
                .arg("-d")
                .arg(extract_dir.to_string_lossy().to_string())
                .output()
                .map_err(|e| format!("解压失败: {}", e))?
        }
    };

    if !extract_result.status.success() {
        let stderr = decode_output(&extract_result.stderr);
        return Err(format!("解压失败: {}", stderr));
    }

    // 验证解压后的入口文件
    let hermes_script = extract_dir.join("hermes");
    if !hermes_script.exists() {
        return Err("解压完成但未找到 hermes 入口文件".to_string());
    }

    // 删除 zip 文件
    std::fs::remove_file(&zip_path).ok();

    // Step 3: 安装 Python 依赖
    let python = find_hermes_python()?;

    // 升级 pip（静默）
    let mut pip_upgrade = new_python_cmd(&python);
    pip_upgrade.arg("-m").arg("pip").arg("install").arg("--upgrade").arg("pip")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    #[cfg(target_os = "windows")]
    pip_upgrade.creation_flags(0x08000000);
    pip_upgrade.output().ok();

    // 安装 Hermes Agent（静默）
    let mut install = new_python_cmd(&python);
    install.arg("-m").arg("pip").arg("install")
        .arg(extract_dir.to_string_lossy().to_string());
    #[cfg(target_os = "windows")]
    install.creation_flags(0x08000000);
    let install_result = install.output()
        .map_err(|e| format!("运行 pip install 失败: {}", e))?;

    if !install_result.status.success() {
        // 尝试直接安装核心依赖（静默）
        let mut fallback = new_python_cmd(&python);
        fallback.arg("-m").arg("pip").arg("install")
            .arg("openai").arg("anthropic").arg("httpx[socks]")
            .arg("rich").arg("fire").arg("tenacity")
            .arg("pyyaml").arg("requests").arg("jinja2")
            .arg("pydantic").arg("prompt_toolkit").arg("python-dotenv");
        #[cfg(target_os = "windows")]
        fallback.creation_flags(0x08000000);
        let fb_result = fallback.output()
            .map_err(|e| format!("安装核心依赖失败: {}", e))?;
        if !fb_result.status.success() {
            let fb_stderr = decode_output(&fb_result.stderr);
            return Err(format!("安装 Python 依赖失败: {}", fb_stderr));
        }
    }

    // Step 4: 安装 Node.js（Web UI 需要）
    ensure_nodejs(&data_dir).await?;

    Ok(serde_json::json!({
        "success": true,
        "message": "环境安装成功",
        "path": hermes_script.to_string_lossy().to_string(),
    }))
}

/// 获取 Node.js 目录（系统或本地捆绑）
fn get_nodejs_dir(data_dir: &Path) -> PathBuf {
    data_dir.join("nodejs")
}

/// 获取可用的 npm 路径（优先系统安装，后备用本地捆绑）
fn find_npm(data_dir: &Path) -> Option<PathBuf> {
    // 检查系统安装
    let system_npm = if cfg!(target_os = "windows") { "npm.cmd" } else { "npm" };
    let mut cmd = std::process::Command::new(system_npm);
    cmd.arg("--version")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    #[cfg(target_os = "windows")]
    cmd.creation_flags(0x08000000);
    if cmd.output().map(|o| o.status.success()).unwrap_or(false) {
        return Some(PathBuf::from(system_npm));
    }

    // 检查本地捆绑
    let bundled = if cfg!(target_os = "windows") {
        get_nodejs_dir(data_dir).join("npm.cmd")
    } else {
        get_nodejs_dir(data_dir).join("bin").join("npm")
    };
    if bundled.exists() {
        return Some(bundled);
    }

    None
}

/// 确保 Node.js 已安装（下载便携版并解压到数据目录）
async fn ensure_nodejs(data_dir: &Path) -> Result<(), String> {
    // 检查系统 Node.js
    let node_name = if cfg!(target_os = "windows") { "node.exe" } else { "node" };
    let mut check = std::process::Command::new(node_name);
    check.arg("--version")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    #[cfg(target_os = "windows")]
    check.creation_flags(0x08000000);
    if check.output().map(|o| o.status.success()).unwrap_or(false) {
        return Ok(());
    }

    // 检查本地已安装
    let nodejs_dir = get_nodejs_dir(data_dir);
    let node_exe = if cfg!(target_os = "windows") {
        nodejs_dir.join("node.exe")
    } else {
        nodejs_dir.join("bin").join("node")
    };
    if node_exe.exists() {
        return Ok(());
    }

    // 需要下载 Node.js
    let node_version = "20.18.0";
    let (download_url, zip_name) = if cfg!(target_os = "windows") {
        (
            format!("https://nodejs.org/dist/v{}/node-v{}-win-x64.zip", node_version, node_version),
            format!("node-v{}-win-x64.zip", node_version),
        )
    } else {
        (
            format!("https://nodejs.org/dist/v{}/node-v{}-darwin-x64.tar.gz", node_version, node_version),
            format!("node-v{}-darwin-x64.tar.gz", node_version),
        )
    };

    let zip_path = data_dir.join(&zip_name);

    // 下载
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(300))
        .build()
        .map_err(|e| format!("HTTP client error: {}", e))?;

    let response = client
        .get(&download_url)
        .send()
        .await
        .map_err(|e| format!("下载 Node.js 失败: {}", e))?;

    if !response.status().is_success() {
        return Err(format!("下载 Node.js 失败: HTTP {}", response.status()));
    }

    let bytes = response
        .bytes()
        .await
        .map_err(|e| format!("读取 Node.js 数据失败: {}", e))?
        .to_vec();

    std::fs::write(&zip_path, &bytes).map_err(|e| format!("写入 Node.js 文件失败: {}", e))?;

    // Node.js 便携版解压后得到 node-vxx.x.x-win-x64/ 目录，需要重命名为 nodejs/
    if cfg!(target_os = "windows") {
        // 解压 ZIP
        #[cfg(target_os = "windows")]
        {
            const CREATE_NO_WINDOW: u32 = 0x08000000;
            let unzip_result = std::process::Command::new("powershell")
                .arg("-NoProfile")
                .arg("-Command")
                .arg(format!(
                    "Expand-Archive -Path '{}' -DestinationPath '{}' -Force",
                    zip_path.to_string_lossy(),
                    data_dir.to_string_lossy()
                ))
                .creation_flags(CREATE_NO_WINDOW)
                .output()
                .map_err(|e| format!("解压 Node.js 失败: {}", e))?;

            if !unzip_result.status.success() {
                let stderr = decode_output(&unzip_result.stderr);
                return Err(format!("解压 Node.js 失败: {}", stderr));
            }
        }

        // 重命名 node-v20.18.0-win-x64 -> nodejs
        let extracted_dir = data_dir.join(format!("node-v{}-win-x64", node_version));
        if extracted_dir.exists() {
            std::fs::rename(&extracted_dir, &nodejs_dir).ok();
        }
    } else {
        // macOS: tar xzf
        // (simplified for now)
    }

    // 清理 zip 文件
    std::fs::remove_file(&zip_path).ok();

    // 验证
    let mut verify = std::process::Command::new(&node_exe);
    verify.arg("--version")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    #[cfg(target_os = "windows")]
    verify.creation_flags(0x08000000);
    if !verify.output().map(|o| o.status.success()).unwrap_or(false) {
        return Err("Node.js 安装验证失败".to_string());
    }

    Ok(())
}

/// 单独安装 Node.js（独立命令，便于 UI 调用）
#[tauri::command]
async fn setup_nodejs() -> Result<serde_json::Value, String> {
    let data_dir = get_data_dir()?;
    ensure_nodejs(&data_dir).await?;
    Ok(serde_json::json!({
        "success": true,
        "message": "Node.js 安装成功",
    }))
}

// ============ Feishu ============

async fn get_feishu_token(app_id: &str, app_secret: &str) -> Result<String, String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| format!("HTTP client error: {}", e))?;

    #[derive(Serialize)]
    struct TokenReq<'a> { app_id: &'a str, app_secret: &'a str }
    #[derive(Deserialize)]
    struct TokenResp { code: i32, msg: String, tenant_access_token: String }

    let resp = client
        .post("https://open.feishu.cn/open-apis/auth/v3/tenant_access_token/internal")
        .json(&TokenReq { app_id, app_secret })
        .send()
        .await
        .map_err(|e| format!("请求飞书 token 失败: {}", e))?;

    let token_resp: TokenResp = resp.json().await.map_err(|_| "解析响应失败")?;
    if token_resp.code != 0 {
        return Err(format!("飞书 token 错误: {}", token_resp.msg));
    }
    Ok(token_resp.tenant_access_token)
}

async fn send_feishu_message(token: &str, chat_id: &str, content: &str) -> Result<(), String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| format!("HTTP client error: {}", e))?;

    #[derive(Serialize)]
    struct MsgReq<'a> {
        receive_id: &'a str,
        msg_type: &'a str,
        content: String,
    }

    let resp = client
        .post("https://open.feishu.cn/open-apis/im/v1/messages?receive_id_type=chat_id")
        .header("Authorization", format!("Bearer {}", token))
        .json(&MsgReq {
            receive_id: chat_id,
            msg_type: "text",
            content: serde_json::json!({"text": content}).to_string(),
        })
        .send()
        .await
        .map_err(|e| format!("发送飞书消息失败: {}", e))?;

    #[derive(Deserialize)]
    struct SendResp { code: i32, msg: String }
    let send_resp: SendResp = resp.json().await.map_err(|_| "解析响应失败")?;
    if send_resp.code != 0 {
        return Err(format!("飞书发送错误: {}", send_resp.msg));
    }
    Ok(())
}

// ============ Dashboard Launch ============

/// 递归复制目录（备用方案，软链接失败时使用）
fn copy_dir_recursive(src: &std::path::Path, dst: &std::path::Path) -> std::io::Result<()> {
    if !dst.exists() {
        std::fs::create_dir_all(dst)?;
    }
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());
        if file_type.is_dir() {
            copy_dir_recursive(&src_path, &dst_path)?;
        } else {
            std::fs::copy(&src_path, &dst_path)?;
        }
    }
    Ok(())
}

/// 为命令设置本地 Node.js PATH（如有捆绑版）
fn setup_node_path(cmd: &mut std::process::Command) {
    if let Ok(data_dir) = get_data_dir() {
        let nodejs_dir = get_nodejs_dir(&data_dir);
        let node_bin = if cfg!(target_os = "windows") { nodejs_dir } else { nodejs_dir.join("bin") };
        if node_bin.exists() {
            let old_path = std::env::var("PATH").unwrap_or_default();
            cmd.env("PATH", format!("{};{}", node_bin.to_string_lossy(), old_path));
        }
    }
}

/// 启动 Hermes Agent Dashboard 并在浏览器中打开
#[tauri::command]
async fn launch_dashboard(app_handle: tauri::AppHandle) -> Result<serde_json::Value, String> {
    let python = find_hermes_python()?;
    let hermes = find_hermes_agent()?;
    let agent_dir = hermes.parent().ok_or("无法获取 Hermes Agent 目录")?;

    // 检查 web_dist 是否存在，构建 web UI 前端
    let web_dist = agent_dir.join("web_dist");
    let web_src = agent_dir.join("web");
    if !web_dist.exists() {
        if web_src.exists() {
            // 找 npm（系统优先，其次本地捆绑）
            let npm = find_npm(&get_data_dir().unwrap_or_default())
                .unwrap_or_else(|| {
                    if cfg!(target_os = "windows") {
                        PathBuf::from("npm.cmd")
                    } else {
                        PathBuf::from("npm")
                    }
                });
            // 先安装依赖（如果 node_modules 不存在）
            let has_modules = web_src.join("node_modules").exists();
            if !has_modules {
                let mut install = std::process::Command::new(&npm);
                install.args(["ci", "--prefer-offline"]).current_dir(&web_src);
                setup_node_path(&mut install);
                #[cfg(target_os = "windows")]
                install.creation_flags(0x08000000);
                let install_out = install.output().map_err(|e| format!("npm install 失败: {}", e))?;
                if !install_out.status.success() {
                    return Err(format!("npm 安装依赖失败: {}", decode_output(&install_out.stderr)));
                }
            }

            // Windows 上 prebuild 脚本使用 rm/cp 等 Unix 命令会失败，需要修复
            #[cfg(target_os = "windows")]
            {
                // 1. 手动执行 sync-assets（Windows 上 prebuild 的 rm -rf 无法运行）
                let mut sync_cmd = std::process::Command::new("powershell");
                sync_cmd
                    .args([
                        "-NoProfile",
                        "-Command",
                        "$f='node_modules/@nous-research/ui/dist/fonts'; \
                         $a='node_modules/@nous-research/ui/dist/assets'; \
                         Remove-Item -Recurse -Force public/fonts,public/ds-assets -ErrorAction SilentlyContinue; \
                         if(Test-Path $f){ \
                           New-Item -ItemType Directory -Force public/fonts|Out-Null; \
                           Get-ChildItem $f|Copy-Item -Destination public/fonts -Recurse -Force \
                         }; \
                         if(Test-Path $a){ \
                           New-Item -ItemType Directory -Force public/ds-assets|Out-Null; \
                           Get-ChildItem $a|Copy-Item -Destination public/ds-assets -Recurse -Force \
                         }",
                    ])
                    .current_dir(&web_src);
                sync_cmd.creation_flags(0x08000000);
                sync_cmd.output().ok();
                // 2. 移除 prebuild 脚本（rm -rf 在 Windows cmd 中不存在）
                let pkg_path = web_src.join("package.json");
                if let Ok(content) = std::fs::read_to_string(&pkg_path) {
                    let patched = content.replace("\"prebuild\": \"npm run sync-assets\",\n", "");
                    if patched != content {
                        let _ = std::fs::write(&pkg_path, patched);
                    }
                }
            }

            let mut build = std::process::Command::new(&npm);
            build.args(["run", "build"]).current_dir(&web_src);
            setup_node_path(&mut build);
            #[cfg(target_os = "windows")]
            build.creation_flags(0x08000000);
            let output = build.output().map_err(|e| format!("构建 Web UI 失败: {}", e))?;
            if !output.status.success() {
                return Err(format!(
                    "构建 Web UI 失败: {}",
                    decode_output(&output.stderr)
                ));
            }
            // 构建输出在 hermes_cli/web_dist/，软链接到 web_dist/
            let built_dist = agent_dir.join("hermes_cli").join("web_dist");
            if built_dist.exists() {
                #[cfg(not(target_os = "windows"))]
                { std::os::unix::fs::symlink(&built_dist, &web_dist).ok(); }
                #[cfg(target_os = "windows")]
                { std::os::windows::fs::symlink_dir(&built_dist, &web_dist).ok(); }
                // 如果软链接失败，直接复制
                if !web_dist.exists() {
                    copy_dir_recursive(&built_dist, &web_dist).ok();
                }
            }
        }
        if !web_dist.exists() {
            return Err(
                "Web UI 文件未找到。请确保运行环境安装完整，或安装 Node.js (nodejs.org) 后重试。"
                    .to_string()
            );
        }
    }

    // 启动 Hermes Dashboard 进程（后台运行）
    let port = 9119;
    let mut cmd = std::process::Command::new(&python);
    cmd.args(["-m", "hermes_cli.main", "dashboard", "--port", &port.to_string(), "--no-open"])
        .env("HERMES_WEB_DIST", web_dist.to_string_lossy().to_string())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    #[cfg(target_os = "windows")]
    cmd.creation_flags(0x08000000);

    let child = cmd.spawn().map_err(|e| format!("启动 Dashboard 失败: {}", e))?;

    // 等待服务就绪（最多 15 秒）
    let url = format!("http://127.0.0.1:{}/api/status", port);
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(3))
        .build()
        .map_err(|_| "HTTP 客户端错误")?;

    let mut ready = false;
    for _ in 0..15 {
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        if client.get(&url).send().await.is_ok() {
            ready = true;
            break;
        }
    }

    if !ready {
        return Err("Dashboard 启动超时".to_string());
    }

    // 在浏览器中打开
    let dashboard_url = format!("http://127.0.0.1:{}", port);
    if let Err(e) = open::that(&dashboard_url) {
        eprintln!("打开浏览器失败: {}", e);
    }

    Ok(serde_json::json!({
        "success": true,
        "url": dashboard_url,
        "pid": child.id(),
    }))
}

// ============ Main ============

fn main() {
    tauri::Builder::default()
        .manage(AppState {
            hermes_ready: Mutex::new(false),
            chat_history: Mutex::new(Vec::new()),
        })
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_shell::init())
        .invoke_handler(tauri::generate_handler![
            get_machine_code,
            get_license_status,
            activate_license,
            deactivate_license,
            generate_activation_code,
            chat_completion,
            chat_stream,
            chat_direct,
            launch_dashboard,
            save_api_config,
            get_api_config,
            save_feishu_config,
            test_feishu,
            check_hermes_environment,
            setup_hermes_environment,
            setup_nodejs,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
