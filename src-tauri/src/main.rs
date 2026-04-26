// Prevents additional console window on Windows in release
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use tauri::Manager;
#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;

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
const DEFAULT_API_BASE: &str = "https://api.minimaxi.com/anthropic";
const LICENSE_SERVER: &str = "http://175.27.242.158:5000";

// 内置 MiniMax API Key（发布前确认额度充足）
const BUILTIN_API_KEY: &str = "sk-cp-_2yFksEQQQrzpyKNpNsPD7fiPiKbsOXJDLTfOwQdWDLZQro_iuG_UUFbrQOn9-g_WJPQtpf-MCx02bv89LYyhy6pI40TjrelWji--aLVNTN6fePCY64Udi0"; // MiniMax-M2.7-highspeed

// ============ State ============

pub struct AppState {
    pub hermes_ready: Mutex<bool>,
}

// ============ Helpers ============

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
                model: "MiniMax-Text-01".to_string(),
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
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|p| p.to_path_buf()))
        .unwrap_or_default();

    // 1. 和 exe 同目录的 python/python.exe
    let bundled = exe_dir.join("python").join("python.exe");
    if bundled.exists() {
        return Ok(bundled);
    }

    // 2. 环境变量 HERMES_PYTHON
    if let Ok(env_py) = std::env::var("HERMES_PYTHON") {
        let p = PathBuf::from(env_py);
        if p.exists() {
            return Ok(p);
        }
    }

    #[cfg(target_os = "windows")]
    {
        // 辅助: 测试 Python 确实能启动 (验证 --version 成功)
        fn python_works(path: &std::path::Path) -> bool {
            std::process::Command::new(path)
                .arg("--version")
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .output()
                .map(|o| o.status.success())
                .unwrap_or(false)
        }

        // 3. py.exe (Python Launcher, C:\Windows\ 下) — 绕过 WindowsApps 执行别名
        //    python.org 安装器默认安装这个启动器，它通过注册表查找真实 Python
        if python_works(Path::new("py.exe")) {
            return Ok(PathBuf::from("py.exe"));
        }

        // 4. Program Files 系统级安装
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
                return Ok(pb);
            }
        }

        // 5. 当前用户的 AppData Python 安装 (Python.org 安装器用户安装默认位置)
        if let Ok(local_app_data) = std::env::var("LOCALAPPDATA") {
            let py_base = PathBuf::from(local_app_data).join("Programs\\Python");
            if py_base.exists() {
                if let Ok(entries) = std::fs::read_dir(&py_base) {
                    for entry in entries.flatten() {
                        let python_exe = entry.path().join("python.exe");
                        if python_exe.exists() && python_works(&python_exe) {
                            return Ok(python_exe);
                        }
                    }
                }
            }
        }

        // 6. where.exe python.exe — 跳过 WindowsApps 存根，验证每个结果
        if let Ok(output) = std::process::Command::new("where.exe")
            .arg("python.exe")
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .output()
        {
            if output.status.success() {
                for line in String::from_utf8_lossy(&output.stdout).lines() {
                    let path = line.trim().to_string();
                    if path.is_empty() || path.contains("WindowsApps") {
                        continue;
                    }
                    let pb = PathBuf::from(&path);
                    if pb.exists() && python_works(&pb) {
                        return Ok(pb);
                    }
                }
            }
        }
    }

    // 7. python.exe/python3（靠系统 PATH 搜索，需验证）
    let system = PathBuf::from(
        if cfg!(target_os = "windows") { "python.exe" } else { "python3" }
    );

    if std::process::Command::new(&system)
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
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|p| p.to_path_buf()))
        .unwrap_or_default();

    // 1. 和 exe 同目录的 hermes-agent/
    let bundled = exe_dir.join("hermes-agent").join("hermes");
    if bundled.exists() {
        return Ok(bundled);
    }

    // 2. 应用数据目录（setup_hermes_environment 下载的位置）
    if let Ok(data_dir) = get_data_dir() {
        let data_path = data_dir.join("hermes-agent").join("hermes");
        if data_path.exists() {
            return Ok(data_path);
        }
    }

    // 3. Mac 开发环境
    let dev_path = PathBuf::from("/Users/laomashitu/hermes-agent/hermes");
    if dev_path.exists() {
        return Ok(dev_path);
    }

    // 4. 环境变量
    if let Ok(env_path) = std::env::var("HERMES_AGENT") {
        let p = PathBuf::from(env_path);
        if p.exists() {
            return Ok(p);
        }
    }

    Err("未找到 Hermes Agent，请先安装运行环境".to_string())
}

fn run_hermes_chat(prompt: &str, session_id: &str) -> Result<(String, String), String> {
    let python = find_hermes_python()?;
    let hermes = find_hermes_agent()?;
    let config = AppConfig::load()?;
    let api_key = config.effective_api_key();

    if api_key.is_empty() {
        return Err("请先在设置中配置 API Key，或联系作者获取内置 Key".to_string());
    }

    let mut cmd = new_python_cmd(&python);

    cmd.env("MINIMAX_API_KEY", &api_key)
        .env("MINIMAX_CN_API_KEY", &api_key)
        .env("HERMES_Q", prompt);
    // 使用 -c 方式直接调用 cli.main()，完全规避 Windows 上无扩展名脚本执行问题
    if let Some(agent_dir) = hermes.parent() {
        let agent_dir_escaped = agent_dir.to_string_lossy().replace('\\', "\\\\").replace('\'', "\\'");
        let resume_arg = if !session_id.is_empty() {
            format!(", resume='{}'", session_id.replace('\'', "\\'"))
        } else {
            String::new()
        };
        let python_code = format!(
            "import sys; sys.path.insert(0, '{}'); import os; from cli import main; main(query=os.environ.get('HERMES_Q',''), quiet=True{})",
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
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        let exit_code = output.status.code().unwrap_or(-1);
        return Err(format!(
            "Hermes Agent 错误 (exit:{})\n----stderr----\n{}\n----stdout----\n{}",
            exit_code, stderr, stdout
        ));
    }

    let raw = String::from_utf8_lossy(&output.stdout).to_string();
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
    if config.api_key.is_empty() && !BUILTIN_API_KEY.is_empty() {
        config.api_key = BUILTIN_API_KEY.to_string();
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

    let mut status = serde_json::json!({
        "python_ok": python_ok,
        "agent_ok": agent_ok,
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
                let version = String::from_utf8_lossy(&out.stdout).trim().to_string();
                status["version"] = serde_json::Value::String(version);
            }
        }
    }

    Ok(status)
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
        let stderr = String::from_utf8_lossy(&extract_result.stderr);
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

    // 升级 pip
    new_python_cmd(&python)
        .arg("-m")
        .arg("pip")
        .arg("install")
        .arg("--upgrade")
        .arg("pip")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .output()
        .ok();

    // 安装 Hermes Agent
    let install = new_python_cmd(&python)
        .arg("-m")
        .arg("pip")
        .arg("install")
        .arg(extract_dir.to_string_lossy().to_string())
        .output()
        .map_err(|e| format!("运行 pip install 失败: {}", e))?;

    if !install.status.success() {
        let stderr = String::from_utf8_lossy(&install.stderr);
        // 尝试直接安装核心依赖
        let fallback = new_python_cmd(&python)
            .arg("-m")
            .arg("pip")
            .arg("install")
            .arg("openai")
            .arg("anthropic")
            .arg("httpx[socks]")
            .arg("rich")
            .arg("fire")
            .arg("tenacity")
            .arg("pyyaml")
            .arg("requests")
            .arg("jinja2")
            .arg("pydantic")
            .arg("prompt_toolkit")
            .arg("python-dotenv")
            .output()
            .map_err(|e| format!("安装核心依赖失败: {}", e))?;
        if !fallback.status.success() {
            let fb_stderr = String::from_utf8_lossy(&fallback.stderr);
            return Err(format!("安装 Python 依赖失败: {}", fb_stderr));
        }
    }

    Ok(serde_json::json!({
        "success": true,
        "message": "环境安装成功",
        "path": hermes_script.to_string_lossy().to_string(),
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

// ============ Main ============

fn main() {
    tauri::Builder::default()
        .manage(AppState {
            hermes_ready: Mutex::new(false),
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
            save_api_config,
            get_api_config,
            save_feishu_config,
            test_feishu,
            check_hermes_environment,
            setup_hermes_environment,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
