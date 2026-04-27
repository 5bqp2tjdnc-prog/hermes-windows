<template>
  <div class="app">
    <!-- Left Sidebar -->
    <aside class="sidebar">
      <div class="sidebar-header">
        <div class="logo-wrap">
          <span class="logo-icon">☤</span>
          <div class="logo-text">
            <h1 class="logo-title">Hermes</h1>
            <span class="logo-subtitle">Agent</span>
          </div>
        </div>
      </div>

      <nav class="sidebar-nav">
        <button
          class="nav-item"
          :class="{ active: currentView === 'chat' }"
          @click="openChatView"
        >
          <svg class="nav-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <path d="M21 15a2 2 0 01-2 2H7l-4 4V5a2 2 0 012-2h14a2 2 0 012 2z"/>
          </svg>
          AI 对话
        </button>
        <button
          class="nav-item"
          :class="{ active: currentView === 'dashboard' }"
          @click="openDashboardView"
          :disabled="!envReady"
        >
          <svg class="nav-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <polyline points="16 3 21 3 21 8"/>
            <line x1="4" y1="20" x2="21" y2="3"/>
            <polyline points="21 16 21 21 16 21"/>
            <line x1="15" y1="15" x2="21" y2="21"/>
            <line x1="4" y1="4" x2="9" y2="9"/>
          </svg>
          管理后台
        </button>
        <button
          class="nav-item"
          :class="{ active: currentView === 'guide' }"
          @click="currentView = 'guide'"
        >
          <svg class="nav-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <circle cx="12" cy="12" r="10"/>
            <line x1="12" y1="16" x2="12" y2="12"/>
            <line x1="12" y1="8" x2="12.01" y2="8"/>
          </svg>
          使用说明
        </button>
        <button
          class="nav-item"
          :class="{ active: currentView === 'settings' }"
          @click="currentView = 'settings'"
        >
          <svg class="nav-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <circle cx="12" cy="12" r="3"/>
            <path d="M19.4 15a1.65 1.65 0 00.33 1.82l.06.06a2 2 0 010 2.83 2 2 0 01-2.83 0l-.06-.06a1.65 1.65 0 00-1.82-.33 1.65 1.65 0 00-1 1.51V21a2 2 0 01-4 0v-.09A1.65 1.65 0 009 19.4a1.65 1.65 0 00-1.82.33l-.06.06a2 2 0 01-2.83-2.83l.06-.06A1.65 1.65 0 004.68 15a1.65 1.65 0 00-1.51-1H3a2 2 0 010-4h.09A1.65 1.65 0 004.6 9a1.65 1.65 0 00-.33-1.82l-.06-.06a2 2 0 012.83-2.83l.06.06A1.65 1.65 0 009 4.68a1.65 1.65 0 001-1.51V3a2 2 0 014 0v.09a1.65 1.65 0 001 1.51 1.65 1.65 0 001.82-.33l.06-.06a2 2 0 012.83 2.83l-.06.06A1.65 1.65 0 0019.4 9a1.65 1.65 0 001.51 1H21a2 2 0 010 4h-.09a1.65 1.65 0 00-1.51 1z"/>
          </svg>
          设置
        </button>
      </nav>

      <div class="sidebar-footer">
        <div class="license-warning" v-if="licenseInfo.activated && licenseInfo.days_left < 30">
          即将到期 · {{ licenseInfo.days_left }} 天
        </div>
      </div>
    </aside>

    <!-- Main Content -->
    <main class="main-content">
      <!-- Toast -->
      <transition name="toast">
        <div v-if="toast.show" class="toast" :class="toast.type">
          <svg v-if="toast.type === 'success'" class="toast-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <path d="M22 11.08V12a10 10 0 11-5.93-9.14"/>
            <polyline points="22 4 12 14.01 9 11.01"/>
          </svg>
          <svg v-else class="toast-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <circle cx="12" cy="12" r="10"/>
            <line x1="12" y1="8" x2="12" y2="12"/>
            <line x1="12" y1="16" x2="12.01" y2="16"/>
          </svg>
          <span>{{ toast.message }}</span>
          <button class="toast-close" @click="toast.show = false">
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <line x1="18" y1="6" x2="6" y2="18"/>
              <line x1="6" y1="6" x2="18" y2="18"/>
            </svg>
          </button>
        </div>
      </transition>
      <!-- AI 对话 (iframe 内嵌) -->
      <div v-if="currentView === 'chat'" class="iframe-view">
        <iframe :src="chatUrl" frameborder="0" class="app-iframe" allow="clipboard-read; clipboard-write"></iframe>
      </div>

      <!-- 管理后台 (iframe 内嵌) -->
      <div v-else-if="currentView === 'dashboard'" class="iframe-view">
        <iframe :src="dashboardUrl" frameborder="0" class="app-iframe"></iframe>
      </div>

      <!-- 使用说明 -->
      <div v-else-if="currentView === 'guide'" class="settings-view">
        <div class="settings-header">
          <h2>
            <svg class="header-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <circle cx="12" cy="12" r="10"/>
              <line x1="12" y1="16" x2="12" y2="12"/>
              <line x1="12" y1="8" x2="12.01" y2="8"/>
            </svg>
            使用说明
          </h2>
        </div>

        <div class="settings-grid guide-grid">
          <section class="settings-card">
            <div class="card-header">
              <svg class="card-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <circle cx="12" cy="12" r="10"/>
                <line x1="12" y1="16" x2="12" y2="12"/>
                <line x1="12" y1="8" x2="12.01" y2="8"/>
              </svg>
              <h3>软件简介</h3>
            </div>
            <div class="card-body">
              <p class="guide-text">
                Hermes Agent 是一款基于 MiniMax M2.7 大模型的 AI 助手软件。支持联网搜索、工具调用和任务执行，提供完整的 AI 对话体验。
              </p>
            </div>
          </section>

          <section class="settings-card">
            <div class="card-header">
              <svg class="card-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <polyline points="22 12 18 12 15 21 9 3 6 12 2 12"/>
              </svg>
              <h3>使用流程</h3>
            </div>
            <div class="card-body">
              <div class="guide-flow">
                <div class="guide-step">
                  <span class="guide-step-num">1</span>
                  <div>
                    <strong>激活许可证</strong>
                    <p class="guide-hint">首次使用需在「设置」页面复制机器码，联系作者获取激活码后激活</p>
                  </div>
                </div>
                <div class="guide-step">
                  <span class="guide-step-num">2</span>
                  <div>
                    <strong>安装运行环境</strong>
                    <p class="guide-hint">在「设置」→「运行环境」中检查 Python 和 Hermes Agent 是否已安装，如未安装点击安装</p>
                  </div>
                </div>
                <div class="guide-step">
                  <span class="guide-step-num">3</span>
                  <div>
                    <strong>开始使用</strong>
                    <p class="guide-hint">环境就绪后，点击左侧「AI 对话」即可开始聊天，支持联网搜索、工具调用等功能</p>
                  </div>
                </div>
              </div>
            </div>
          </section>

          <section class="settings-card">
            <div class="card-header">
              <svg class="card-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <circle cx="12" cy="12" r="3"/>
                <path d="M19.4 15a1.65 1.65 0 00.33 1.82l.06.06a2 2 0 010 2.83 2 2 0 01-2.83 0l-.06-.06a1.65 1.65 0 00-1.82-.33 1.65 1.65 0 00-1 1.51V21a2 2 0 01-4 0v-.09A1.65 1.65 0 009 19.4a1.65 1.65 0 00-1.82.33l-.06.06a2 2 0 01-2.83-2.83l.06-.06A1.65 1.65 0 004.68 15a1.65 1.65 0 00-1.51-1H3a2 2 0 010-4h.09A1.65 1.65 0 004.6 9a1.65 1.65 0 00-.33-1.82l-.06-.06a2 2 0 012.83-2.83l.06.06A1.65 1.65 0 009 4.68a1.65 1.65 0 001-1.51V3a2 2 0 014 0v.09a1.65 1.65 0 001 1.51 1.65 1.65 0 001.82-.33l.06-.06a2 2 0 012.83 2.83l-.06.06A1.65 1.65 0 0019.4 9a1.65 1.65 0 001.51 1H21a2 2 0 010 4h-.09a1.65 1.65 0 00-1.51 1z"/>
              </svg>
              <h3>如何修改设置</h3>
            </div>
            <div class="card-body">
              <ul class="guide-list">
                <li><strong>API 配置</strong>：在「设置」→「API 配置」中可以配置 API Key 和模型。留空则使用内置 MiniMax Key，填入自己的 Key 则使用自配 API</li>
                <li><strong>模型选择</strong>：默认为 MiniMax-M2.7-HighSpeed，可在 API 配置的下拉菜单中切换</li>
                <li><strong>运行环境</strong>：在「设置」→「运行环境」中可检查和管理 Python、Hermes Agent、Node.js 等依赖</li>
                <li><strong>飞书推送</strong>：支持配置飞书机器人，用于消息推送</li>
                <li><strong>许可证管理</strong>：在「设置」→「许可证」中可查看激活状态、到期时间和管理许可证</li>
              </ul>
            </div>
          </section>

          <section class="settings-card">
            <div class="card-header">
              <svg class="card-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <path d="M4 4h16c1.1 0 2 .9 2 2v12c0 1.1-.9 2-2 2H4c-1.1 0-2-.9-2-2V6c0-1.1.9-2 2-2z"/>
                <polyline points="22,6 12,13 2,6"/>
              </svg>
              <h3>联系我们</h3>
            </div>
            <div class="card-body">
              <p class="guide-text">
                如有任何问题，请通过微信联系：
              </p>
              <div class="guide-contact">
                <span class="contact-label">微信：</span>
                <span class="contact-value">13213181166</span>
              </div>
            </div>
          </section>
        </div>
      </div>

      <!-- Settings View -->
      <div v-else-if="currentView === 'settings'" class="settings-view">
        <div class="settings-header">
          <h2>
            <svg class="header-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <circle cx="12" cy="12" r="3"/>
              <path d="M19.4 15a1.65 1.65 0 00.33 1.82l.06.06a2 2 0 010 2.83 2 2 0 01-2.83 0l-.06-.06a1.65 1.65 0 00-1.82-.33 1.65 1.65 0 00-1 1.51V21a2 2 0 01-4 0v-.09A1.65 1.65 0 009 19.4a1.65 1.65 0 00-1.82.33l-.06.06a2 2 0 01-2.83-2.83l.06-.06A1.65 1.65 0 004.68 15a1.65 1.65 0 00-1.51-1H3a2 2 0 010-4h.09A1.65 1.65 0 004.6 9a1.65 1.65 0 00-.33-1.82l-.06-.06a2 2 0 012.83-2.83l.06.06A1.65 1.65 0 009 4.68a1.65 1.65 0 001-1.51V3a2 2 0 014 0v.09a1.65 1.65 0 001 1.51 1.65 1.65 0 001.82-.33l.06-.06a2 2 0 012.83 2.83l-.06.06A1.65 1.65 0 0019.4 9a1.65 1.65 0 001.51 1H21a2 2 0 010 4h-.09a1.65 1.65 0 00-1.51 1z"/>
            </svg>
            设置
          </h2>
        </div>

        <div class="settings-grid">
          <!-- License Section -->
          <section class="settings-card">
            <div class="card-header">
              <svg class="card-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <rect x="3" y="11" width="18" height="11" rx="2" ry="2"/>
                <path d="M7 11V7a5 5 0 0110 0v4"/>
              </svg>
              <h3>许可证</h3>
            </div>

            <div v-if="!licenseInfo.activated" class="card-body">
              <div class="license-flow">
                <div class="flow-step">
                  <span class="step-num">1</span>
                  <div class="step-content">
                    <span class="step-label">复制机器码</span>
                    <div class="machine-code-box">
                      <code>{{ machineCode || '加载中...' }}</code>
                      <button v-if="machineCode" @click="copyMachineCode" class="icon-btn" title="复制">
                        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                          <rect x="9" y="9" width="13" height="13" rx="2" ry="2"/>
                          <path d="M5 15H4a2 2 0 01-2-2V4a2 2 0 012-2h9a2 2 0 012 2v1"/>
                        </svg>
                      </button>
                    </div>
                  </div>
                </div>
                <div class="flow-step">
                  <span class="step-num">2</span>
                  <div class="step-content">
                    <span class="step-label">联系作者获取激活码</span>
                    <span class="step-hint">微信/电话：13213181166</span>
                  </div>
                </div>
                <div class="flow-step">
                  <span class="step-num">3</span>
                  <div class="step-content">
                    <span class="step-label">输入激活码</span>
                    <input
                      v-model="activationCodeInput"
                      type="text"
                      placeholder="HA-XXXX-XXXX-XXXX-XXXX"
                      class="code-input"
                    />
                  </div>
                </div>
              </div>

              <button
                class="btn btn-primary"
                @click="doActivate"
                :disabled="!activationCodeInput.trim() || isActivating"
              >
                <span v-if="isActivating" class="btn-spinner"></span>
                {{ isActivating ? '验证中...' : '激活' }}
              </button>

              <p v-if="activationMessage" class="activation-msg" :class="activationSuccess ? 'success' : 'error'">
                {{ activationMessage }}
              </p>
            </div>

            <div v-else class="card-body">
              <div class="license-info-list">
                <div class="info-item">
                  <span class="info-label">状态</span>
                  <span class="info-value activated-text">
                    <span class="status-dot active-dot"></span>
                    已激活
                  </span>
                </div>
                <div class="info-item">
                  <span class="info-label">到期日期</span>
                  <span class="info-value">{{ licenseInfo.expiry_date || '永久' }}</span>
                </div>
                <div class="info-item">
                  <span class="info-label">剩余天数</span>
                  <span class="info-value" :class="{ warn: licenseInfo.days_left < 30 }">
                    {{ licenseInfo.days_left }} 天
                  </span>
                </div>
                <div class="info-item">
                  <span class="info-label">激活码</span>
                  <span class="info-value mono">{{ licenseInfo.license_key?.substring(0, 20) }}...</span>
                </div>
                <div class="info-item">
                  <span class="info-label">联系方式</span>
                  <span class="info-value" style="color: var(--gold);">微信/电话：13213181166</span>
                </div>
              </div>
              <button class="btn btn-danger" @click="doDeactivate">注销许可证</button>
            </div>
          </section>

          <!-- API Config Section -->
          <section class="settings-card">
            <div class="card-header">
              <svg class="card-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <polyline points="16 3 21 3 21 8"/>
                <line x1="4" y1="20" x2="21" y2="3"/>
                <polyline points="21 16 21 21 16 21"/>
                <line x1="15" y1="15" x2="21" y2="21"/>
                <line x1="4" y1="4" x2="9" y2="9"/>
              </svg>
              <h3>API 配置</h3>
            </div>
            <div class="card-body">
              <div class="form-group">
                <label>API Key</label>
                <div class="input-with-toggle">
                  <input
                    v-model="apiConfig.api_key"
                    type="password"
                    placeholder="留空则使用内置 Key"
                    readonly
                    onfocus="this.removeAttribute('readonly')"
                  />
                </div>
                <span class="form-hint">留空使用内置 Key，填入自己的 Key 则使用自配 API</span>
              </div>
              <div class="form-group">
                <label>API 地址</label>
                <input v-model="apiConfig.api_base" type="text" placeholder="https://mcp.mxai.cn/mcp/api/" />
              </div>
              <div class="form-group">
                <label>模型</label>
                <select v-model="apiConfig.model">
                  <option value="MiniMax-M2.7-highspeed">MiniMax-M2.7-HighSpeed（默认）</option>
                  <option value="MiniMax-Text-01">MiniMax-Text-01</option>
                  <option value="OpenAI/gpt-4o">OpenAI GPT-4o</option>
                  <option value="claude-sonnet-4">Claude Sonnet 4</option>
                  <option value="claude-opus-4">Claude Opus 4</option>
                </select>
              </div>
              <button class="btn btn-primary" @click="saveApiConfig">
                <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                  <path d="M19 21H5a2 2 0 01-2-2V5a2 2 0 012-2h11l5 5v11a2 2 0 01-2 2z"/>
                  <polyline points="17 21 17 13 7 13 7 21"/>
                  <polyline points="7 3 7 8 15 8"/>
                </svg>
                保存配置
              </button>
            </div>
          </section>

          <!-- Environment Section -->
          <section class="settings-card">
            <div class="card-header">
              <svg class="card-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <rect x="2" y="3" width="20" height="14" rx="2" ry="2"/>
                <line x1="8" y1="21" x2="16" y2="21"/>
                <line x1="12" y1="17" x2="12" y2="21"/>
              </svg>
              <h3>运行环境</h3>
            </div>
            <div class="card-body">
              <div class="env-check-list">
                <div class="env-item">
                  <span class="env-check-label">Python</span>
                  <span class="env-check-status" :class="envStatus.python_ok ? 'ok' : 'fail'">
                    <span class="check-dot"></span>
                    {{ envStatus.python_ok ? '已找到' : '未找到' }}
                  </span>
                  <span class="env-check-path">{{ envStatus.python_path || '-' }}</span>
                </div>
                <div class="env-item" v-if="!envStatus.python_ok">
                  <span class="env-hint">首次使用需安装 Python 3.11+</span>
                  <a class="btn btn-sm btn-outline" href="https://www.python.org/downloads/" target="_blank">
                    下载 Python
                  </a>
                </div>
                <div class="env-item">
                  <span class="env-check-label">Hermes Agent</span>
                  <span class="env-check-status" :class="envStatus.agent_ok ? 'ok' : 'fail'">
                    <span class="check-dot"></span>
                    {{ envStatus.agent_ok ? '已找到' : '未找到' }}
                  </span>
                  <span class="env-check-path">{{ envStatus.agent_path || '-' }}</span>
                </div>
                <div class="env-item">
                  <span class="env-check-label">Node.js</span>
                  <span class="env-check-status" :class="envStatus.node_ok ? 'ok' : 'fail'">
                    <span class="check-dot"></span>
                    {{ envStatus.node_ok ? '已安装' : '未安装' }}
                  </span>
                  <span class="env-check-path">{{ envStatus.node_version || '管理后台需要' }}</span>
                </div>
                <div class="env-item" v-if="!envStatus.node_ok">
                  <span class="env-hint">管理后台需要 Node.js，可自动下载安装</span>
                  <button class="btn btn-sm btn-primary" @click="setupNodejs" :disabled="isInstallingNode">
                    {{ isInstallingNode ? '安装中...' : '安装 Node.js' }}
                  </button>
                </div>
                <div class="env-item" v-if="envStatus.version">
                  <span class="env-check-label">版本</span>
                  <span class="env-check-status ok">{{ envStatus.version }}</span>
                </div>
              </div>
              <div class="env-actions">
                <button class="btn btn-secondary" @click="checkEnvironment" :disabled="envChecking">
                  <span v-if="envChecking" class="btn-spinner"></span>
                  重新检查
                </button>
                <button v-if="!envStatus.agent_ok" class="btn btn-primary" @click="setupEnvironment" :disabled="isSettingUp || !envStatus.python_ok">
                  <span v-if="isSettingUp" class="btn-spinner"></span>
                  {{ isSettingUp ? '安装中...' : '下载安装 Hermes Agent' }}
                </button>
              </div>
              <div class="env-overall" :class="envStatus.ready ? 'ready' : 'not-ready'">
                <span class="overall-dot"></span>
                {{ envStatus.ready ? '运行环境就绪，可以开始对话' : '运行环境未就绪' }}
              </div>
            </div>
          </section>

          <!-- Feishu Section -->
          <section class="settings-card">
            <div class="card-header">
              <svg class="card-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <path d="M4 4h16c1.1 0 2 .9 2 2v12c0 1.1-.9 2-2 2H4c-1.1 0-2-.9-2-2V6c0-1.1.9-2 2-2z"/>
                <polyline points="22,6 12,13 2,6"/>
              </svg>
              <h3>飞书推送</h3>
            </div>
            <div class="card-body">
              <div class="form-group">
                <label>App ID</label>
                <input v-model="apiConfig.feishu_app_id" type="text" placeholder="cli_xxx" />
              </div>
              <div class="form-group">
                <label>App Secret</label>
                <input v-model="apiConfig.feishu_app_secret" type="password" placeholder="App Secret" />
              </div>
              <div class="form-group">
                <label>群 ID</label>
                <input v-model="apiConfig.feishu_chat_id" type="text" placeholder="oc_xxx" />
              </div>
              <div class="btn-row">
                <button class="btn btn-primary" @click="saveFeishuConfig">
                  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                    <path d="M19 21H5a2 2 0 01-2-2V5a2 2 0 012-2h11l5 5v11a2 2 0 01-2 2z"/>
                    <polyline points="17 21 17 13 7 13 7 21"/>
                    <polyline points="7 3 7 8 15 8"/>
                  </svg>
                  保存
                </button>
                <button class="btn btn-secondary" @click="testFeishu" :disabled="!apiConfig.feishu_app_id || !apiConfig.feishu_chat_id">
                  发送测试
                </button>
              </div>
            </div>
          </section>
        </div>
      </div>
    </main>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted } from 'vue'
import { invoke } from '@tauri-apps/api/core'

// ============ State ============
type View = 'chat' | 'dashboard' | 'guide' | 'settings'
const currentView = ref<View>('guide')
// URLs (filled by invoke calls)
const chatUrl = ref('http://127.0.0.1:9122')
const dashboardUrl = ref('http://127.0.0.1:9119')

// License
const machineCode = ref('')
const activationCodeInput = ref('')
const isActivating = ref(false)
const activationMessage = ref('')
const activationSuccess = ref(false)
const licenseInfo = ref({
  activated: false,
  machine_code: '',
  expiry_date: null as string | null,
  days_left: 0,
  license_key: '',
})

// ============ Toast ============
const toast = ref({ show: false, message: '', type: 'info' as 'success' | 'error' | 'info' })
let toastTimer: ReturnType<typeof setTimeout> | null = null

function showToast(message: string, type: 'success' | 'error' | 'info' = 'info', duration = 3000) {
  if (toastTimer) clearTimeout(toastTimer)
  toast.value = { show: true, message, type }
  toastTimer = setTimeout(() => {
    toast.value.show = false
  }, duration)
}

// API Config
const apiConfig = ref({
  api_key: '',
  api_base: 'https://api.minimaxi.com/v1',
  model: 'MiniMax-M2.7-highspeed',
  feishu_app_id: '',
  feishu_app_secret: '',
  feishu_chat_id: '',
})

// Environment
const envChecking = ref(false)
const isSettingUp = ref(false)
const isInstallingNode = ref(false)
const envStatus = ref({
  python_ok: false,
  agent_ok: false,
  node_ok: false,
  node_version: '',
  python_path: '',
  agent_path: '',
  version: '',
  ready: false,
})

// ============ Computed ============
const envReady = computed(() => envStatus.value.ready)

// ============ License ============
async function loadLicenseInfo() {
  try {
    licenseInfo.value = await invoke('get_license_status')
    machineCode.value = licenseInfo.value.machine_code || ''
  } catch (e) {
    console.error('Failed to load license:', e)
  }
}

async function loadApiConfig() {
  try {
    const cfg: any = await invoke('get_api_config')
    apiConfig.value = cfg
  } catch (e) {
    console.error('Failed to load API config:', e)
  }
}

async function doActivate() {
  if (!activationCodeInput.value.trim()) return
  isActivating.value = true
  activationMessage.value = ''
  try {
    const result: any = await invoke('activate_license', {
      activationCode: activationCodeInput.value.trim(),
    })
    if (result.success) {
      activationSuccess.value = true
      activationMessage.value = result.message + (result.expiry_date ? ` 到期: ${result.expiry_date.substring(0, 10)}` : '')
      await loadLicenseInfo()
    } else {
      activationSuccess.value = false
      activationMessage.value = result.message
    }
  } catch (e: any) {
    activationSuccess.value = false
    activationMessage.value = e.toString()
  } finally {
    isActivating.value = false
  }
}

async function doDeactivate() {
  try {
    await invoke('deactivate_license')
    licenseInfo.value = { activated: false, machine_code: '', expiry_date: null, days_left: 0, license_key: '' }
    machineCode.value = ''
    activationCodeInput.value = ''
    showToast('许可证已注销', 'success')
  } catch (e: any) {
    showToast('注销失败: ' + e, 'error')
  }
}

async function copyMachineCode() {
  try {
    await navigator.clipboard.writeText(machineCode.value)
    showToast('机器码已复制', 'success')
  } catch (e) {
    showToast('复制失败', 'error')
  }
}

// ============ View Navigation ============
async function openChatView() {
  currentView.value = 'chat'
  try {
    const result: any = await invoke('open_chat_window')
    if (result.url) {
      chatUrl.value = result.url
    }
  } catch (e: any) {
    showToast('启动对话失败: ' + e, 'error')
    currentView.value = 'guide'
  }
}

async function openDashboardView() {
  currentView.value = 'dashboard'
  try {
    const result: any = await invoke('open_management_backend')
    if (result.url) {
      dashboardUrl.value = result.url
    }
  } catch (e: any) {
    showToast('启动管理后台失败: ' + e, 'error')
    currentView.value = 'guide'
  }
}

// ============ Settings ============
async function saveApiConfig() {
  try {
    await invoke('save_api_config', {
      apiKey: apiConfig.value.api_key,
      apiBase: apiConfig.value.api_base,
      model: apiConfig.value.model,
    })
    showToast('API 配置已保存', 'success')
  } catch (e: any) {
    showToast('保存失败: ' + e, 'error')
  }
}

async function saveFeishuConfig() {
  try {
    await invoke('save_feishu_config', {
      appId: apiConfig.value.feishu_app_id,
      appSecret: apiConfig.value.feishu_app_secret,
      chatId: apiConfig.value.feishu_chat_id,
    })
    showToast('飞书配置已保存', 'success')
  } catch (e: any) {
    showToast('保存失败: ' + e, 'error')
  }
}

async function testFeishu() {
  try {
    await invoke('test_feishu')
    showToast('飞书测试消息发送成功', 'success')
  } catch (e: any) {
    showToast('发送失败: ' + e, 'error')
  }
}

async function checkEnvironment() {
  envChecking.value = true
  try {
    const result: any = await invoke('check_hermes_environment')
    envStatus.value = result
    if (result.ready) {
      showToast('运行环境就绪', 'success')
    } else {
      showToast('运行环境未就绪，请检查 Python 和 Hermes Agent 安装', 'error')
    }
  } catch (e: any) {
    showToast('环境检查失败: ' + e, 'error')
  } finally {
    envChecking.value = false
  }
}

async function setupEnvironment() {
  isSettingUp.value = true
  try {
    const result: any = await invoke('setup_hermes_environment')
    showToast(result.message, 'success')
    // 安装后重新检查环境
    await checkEnvironment()
  } catch (e: any) {
    showToast('安装失败: ' + e, 'error')
  } finally {
    isSettingUp.value = false
  }
}

async function setupNodejs() {
  isInstallingNode.value = true
  try {
    const result: any = await invoke('setup_nodejs')
    showToast(result.message, 'success')
    await checkEnvironment()
  } catch (e: any) {
    showToast('安装 Node.js 失败: ' + e, 'error')
  } finally {
    isInstallingNode.value = false
  }
}

// ============ Init ============
onMounted(async () => {
  await Promise.all([loadLicenseInfo(), loadApiConfig()])
  checkEnvironment()
})
</script>

<style>
/* ============ Reset & Base ============ */
*, *::before, *::after {
  margin: 0;
  padding: 0;
  box-sizing: border-box;
}

:root {
  --bg-deep: #041c1c;
  --bg-sidebar: #062626;
  --bg-surface: #0a2e2e;
  --bg-card: #0d3535;
  --bg-input: #0f3a3a;
  --bg-hover: #124040;

  --gold: #FFD700;
  --gold-dim: #c8a800;
  --gold-glow: rgba(255, 189, 56, 0.25);
  --gold-light: #ffe6cb;

  --text-primary: #e8e0d0;
  --text-secondary: #a09880;
  --text-muted: #706858;

  --accent: #FFD700;
  --accent-hover: #e6c200;

  --success: #4ade80;
  --success-bg: rgba(74, 222, 128, 0.1);
  --error: #f87171;
  --error-bg: rgba(248, 113, 113, 0.1);
  --warn: #fbbf24;
  --warn-bg: rgba(251, 191, 36, 0.1);

  --border: #1a3a3a;
  --border-light: #255050;

  --radius-sm: 6px;
  --radius-md: 8px;
  --radius-lg: 12px;

  --font-sans: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, 'Noto Sans SC', sans-serif;
  --font-mono: 'JetBrains Mono', 'SF Mono', Monaco, 'Cascadia Code', monospace;
}

html, body {
  font-family: var(--font-sans);
  background: var(--bg-deep);
  color: var(--text-primary);
  height: 100vh;
  overflow: hidden;
  -webkit-font-smoothing: antialiased;
}

/* Scrollbar */
::-webkit-scrollbar { width: 6px; }
::-webkit-scrollbar-track { background: transparent; }
::-webkit-scrollbar-thumb { background: var(--border); border-radius: 3px; }
::-webkit-scrollbar-thumb:hover { background: var(--border-light); }

/* ============ Layout ============ */
.app {
  display: flex;
  height: 100vh;
  position: relative;
}

/* ============ Toast ============ */
.toast {
  position: fixed;
  top: 16px;
  right: 16px;
  z-index: 9999;
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 12px 16px;
  border-radius: var(--radius-md);
  font-size: 13px;
  line-height: 1.4;
  max-width: 400px;
  box-shadow: 0 8px 24px rgba(0, 0, 0, 0.4);
  border: 1px solid;
}

.toast.success {
  background: rgba(74, 222, 128, 0.12);
  border-color: rgba(74, 222, 128, 0.3);
  color: var(--success);
}

.toast.error {
  background: rgba(248, 113, 113, 0.12);
  border-color: rgba(248, 113, 113, 0.3);
  color: var(--error);
}

.toast.info {
  background: rgba(255, 215, 0, 0.1);
  border-color: rgba(255, 215, 0, 0.2);
  color: var(--gold);
}

.toast-icon {
  width: 18px;
  height: 18px;
  flex-shrink: 0;
}

.toast-close {
  width: 24px;
  height: 24px;
  border-radius: 4px;
  border: none;
  background: transparent;
  color: inherit;
  opacity: 0.5;
  cursor: pointer;
  display: flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
  margin-left: auto;
  transition: opacity 0.15s;
}

.toast-close:hover {
  opacity: 1;
}

.toast-close svg {
  width: 14px;
  height: 14px;
}

.toast-enter-active,
.toast-leave-active {
  transition: all 0.25s ease;
}

.toast-enter-from {
  opacity: 0;
  transform: translateX(40px);
}

.toast-leave-to {
  opacity: 0;
  transform: translateX(40px);
}

/* ============ Sidebar ============ */
.sidebar {
  width: 220px;
  background: var(--bg-sidebar);
  border-right: 1px solid var(--border);
  display: flex;
  flex-direction: column;
  flex-shrink: 0;
}

.sidebar-header {
  padding: 20px 16px;
  border-bottom: 1px solid var(--border);
  display: flex;
  align-items: center;
  justify-content: space-between;
}

.logo-wrap {
  display: flex;
  align-items: center;
  gap: 10px;
}

.logo-icon {
  font-size: 28px;
  line-height: 1;
  filter: drop-shadow(0 0 6px var(--gold-glow));
}

.logo-title {
  font-size: 18px;
  font-weight: 700;
  color: var(--gold);
  line-height: 1.2;
}

.logo-subtitle {
  font-size: 11px;
  color: var(--text-muted);
  text-transform: uppercase;
  letter-spacing: 1px;
}


.sidebar-nav {
  flex: 1;
  padding: 12px 8px;
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.nav-item {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 10px 12px;
  border-radius: var(--radius-md);
  border: none;
  background: transparent;
  color: var(--text-secondary);
  font-size: 13px;
  cursor: pointer;
  text-align: left;
  transition: all 0.15s;
  font-family: inherit;
}

.nav-item:hover {
  background: var(--bg-hover);
  color: var(--text-primary);
}

.nav-item.active {
  background: rgba(255, 215, 0, 0.1);
  color: var(--gold);
}

.nav-icon {
  width: 18px;
  height: 18px;
  flex-shrink: 0;
}

.sidebar-footer {
  padding: 12px;
  border-top: 1px solid var(--border);
  display: flex;
  flex-direction: column;
  gap: 8px;
  align-items: center;
}

.license-warning {
  font-size: 12px;
  color: var(--warn);
  text-align: center;
  padding: 4px 8px;
  background: var(--warn-bg);
  border-radius: var(--radius-sm);
}

/* ============ Main Content ============ */
.main-content {
  flex: 1;
  overflow: hidden;
  min-width: 0;
}

/* ============ Chat View ============ */
.chat-view {
  display: flex;
  flex-direction: column;
  height: 100vh;
  position: relative;
}

.chat-header {
  padding: 14px 20px;
  border-bottom: 1px solid var(--border);
  display: flex;
  align-items: center;
  justify-content: space-between;
}

.chat-header-left h2 {
  font-size: 16px;
  font-weight: 600;
  display: flex;
  align-items: center;
  gap: 8px;
}

.header-icon {
  width: 20px;
  height: 20px;
  color: var(--gold);
}

.chat-header-right {
  display: flex;
  align-items: center;
  gap: 8px;
}

.model-badge,
.session-badge {
  font-size: 11px;
  padding: 4px 10px;
  border-radius: 20px;
  display: flex;
  align-items: center;
  gap: 4px;
}

.model-badge {
  background: rgba(255, 215, 0, 0.1);
  color: var(--gold);
  border: 1px solid rgba(255, 215, 0, 0.2);
}

.session-badge {
  background: var(--bg-card);
  color: var(--text-secondary);
  border: 1px solid var(--border);
  font-family: var(--font-mono);
  font-size: 10px;
  cursor: default;
}

.badge-icon {
  width: 12px;
  height: 12px;
}

.new-chat-btn {
  width: 32px;
  height: 32px;
  border-radius: var(--radius-md);
  border: 1px solid var(--border);
  background: var(--bg-card);
  color: var(--text-secondary);
  cursor: pointer;
  display: flex;
  align-items: center;
  justify-content: center;
  transition: all 0.15s;
}

.new-chat-btn:hover {
  background: var(--bg-hover);
  color: var(--gold);
  border-color: var(--gold-dim);
}

.new-chat-btn svg {
  width: 16px;
  height: 16px;
}

/* Messages Area */
.chat-messages {
  flex: 1;
  overflow-y: auto;
  padding: 20px;
  display: flex;
  flex-direction: column;
  gap: 16px;
}

/* Welcome Screen */
.welcome-screen {
  flex: 1;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 12px;
  padding: 40px 20px;
}

.welcome-logo {
  width: 80px;
  height: 80px;
  border-radius: 50%;
  background: rgba(255, 215, 0, 0.05);
  border: 1px solid rgba(255, 215, 0, 0.15);
  display: flex;
  align-items: center;
  justify-content: center;
  margin-bottom: 8px;
}

.welcome-caduceus {
  font-size: 40px;
  line-height: 1;
  filter: drop-shadow(0 0 10px var(--gold-glow));
}

.welcome-title {
  font-size: 22px;
  font-weight: 700;
  color: var(--gold);
}

.welcome-desc {
  font-size: 13px;
  color: var(--text-secondary);
  margin-bottom: 16px;
}

.welcome-cards {
  display: flex;
  gap: 12px;
  flex-wrap: wrap;
  justify-content: center;
  max-width: 480px;
}

.welcome-card {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 8px 14px;
  background: var(--bg-card);
  border: 1px solid var(--border);
  border-radius: var(--radius-md);
  font-size: 12px;
  color: var(--text-secondary);
}

.card-icon {
  width: 16px;
  height: 16px;
  color: var(--gold);
  flex-shrink: 0;
}

.card-icon svg {
  width: 100%;
  height: 100%;
}

.welcome-tip {
  font-size: 12px;
  color: var(--text-muted);
  margin-top: 16px;
}

/* Message Bubbles */
.message {
  display: flex;
  gap: 12px;
  max-width: 85%;
}

.message.user {
  align-self: flex-end;
  flex-direction: row-reverse;
}

.message.assistant {
  align-self: flex-start;
}

.msg-avatar {
  width: 32px;
  height: 32px;
  border-radius: 50%;
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 16px;
  flex-shrink: 0;
}

.message.user .msg-avatar {
  background: rgba(255, 215, 0, 0.15);
  border: 1px solid rgba(255, 215, 0, 0.2);
}

.message.assistant .msg-avatar {
  background: var(--bg-card);
  border: 1px solid var(--border);
}

.hermes-avatar {
  font-size: 18px;
  line-height: 1;
}

.msg-body {
  min-width: 0;
}

.msg-name {
  font-size: 11px;
  color: var(--text-muted);
  margin-bottom: 4px;
}

.msg-content {
  background: var(--bg-card);
  padding: 12px 16px;
  border-radius: var(--radius-lg);
  border: 1px solid var(--border);
  font-size: 14px;
  line-height: 1.7;
  white-space: pre-wrap;
  word-wrap: break-word;
}

.message.user .msg-content {
  background: rgba(255, 215, 0, 0.08);
  border-color: rgba(255, 215, 0, 0.2);
}

.msg-error {
  margin-top: 6px;
  padding: 8px 12px;
  background: var(--error-bg);
  border: 1px solid rgba(248, 113, 113, 0.2);
  border-radius: var(--radius-md);
  font-size: 12px;
  color: var(--error);
  display: flex;
  align-items: flex-start;
  gap: 6px;
}

.msg-error svg {
  width: 14px;
  height: 14px;
  flex-shrink: 0;
  margin-top: 1px;
}

/* Typing Indicator */
.loading-status {
  font-size: 12px;
  color: var(--text-muted);
  margin-bottom: 6px;
  animation: pulse-text 2s ease-in-out infinite;
}

@keyframes pulse-text {
  0%, 100% { opacity: 0.7; }
  50% { opacity: 1; }
}

/* Streaming Log */
.streaming-log {
  max-height: 360px;
  overflow-y: auto;
  margin-bottom: 8px;
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  background: rgba(0, 0, 0, 0.2);
  padding: 6px;
}

.stream-line {
  font-family: var(--font-mono);
  font-size: 11px;
  line-height: 1.5;
  color: var(--text-secondary);
  padding: 1px 4px;
  word-break: break-all;
  white-space: pre-wrap;
}

.stream-line:not(:last-child) {
  border-bottom: 1px solid rgba(255, 255, 255, 0.03);
}

.typing-indicator {
  display: flex;
  gap: 4px;
  padding: 8px 4px;
}

.typing-indicator span {
  width: 8px;
  height: 8px;
  background: var(--gold);
  border-radius: 50%;
  animation: typing 1.4s infinite;
  opacity: 0.3;
}

.typing-indicator span:nth-child(2) { animation-delay: 0.2s; }
.typing-indicator span:nth-child(3) { animation-delay: 0.4s; }

@keyframes typing {
  0%, 60%, 100% { transform: translateY(0); opacity: 0.3; }
  30% { transform: translateY(-6px); opacity: 1; }
}

/* Chat Input */
.chat-input-area {
  padding: 12px 20px 16px;
  border-top: 1px solid var(--border);
  position: relative;
}

.input-wrapper {
  display: flex;
  gap: 8px;
  align-items: flex-end;
  background: var(--bg-card);
  border: 1px solid var(--border);
  border-radius: var(--radius-lg);
  padding: 4px;
  transition: border-color 0.15s;
}

.input-wrapper:focus-within {
  border-color: var(--gold-dim);
  box-shadow: 0 0 0 1px rgba(255, 215, 0, 0.1);
}

.input-wrapper textarea {
  flex: 1;
  background: transparent;
  border: none;
  padding: 8px 12px;
  color: var(--text-primary);
  font-size: 14px;
  font-family: inherit;
  resize: none;
  min-height: 40px;
  max-height: 120px;
  outline: none;
  line-height: 1.5;
}

.input-wrapper textarea::placeholder {
  color: var(--text-muted);
}

.input-wrapper textarea:disabled {
  opacity: 0.4;
  cursor: not-allowed;
}

.send-btn {
  width: 36px;
  height: 36px;
  border-radius: var(--radius-md);
  border: none;
  background: var(--gold);
  color: #000;
  cursor: pointer;
  display: flex;
  align-items: center;
  justify-content: center;
  transition: all 0.15s;
  flex-shrink: 0;
}

.send-btn:hover:not(:disabled) {
  background: var(--accent-hover);
  transform: scale(1.05);
}

.send-btn:disabled {
  opacity: 0.3;
  cursor: not-allowed;
  background: var(--border-light);
  color: var(--text-muted);
}

.send-btn svg {
  width: 18px;
  height: 18px;
}

.input-overlay {
  position: absolute;
  left: 20px;
  right: 20px;
  bottom: 16px;
  background: var(--bg-card);
  border: 1px solid var(--border);
  border-radius: var(--radius-lg);
  padding: 14px 16px;
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 13px;
  color: var(--text-secondary);
  backdrop-filter: blur(8px);
}

.lock-icon {
  width: 16px;
  height: 16px;
  flex-shrink: 0;
  color: var(--gold-dim);
}

.goto-btn {
  margin-left: auto;
  padding: 6px 14px;
  background: var(--gold);
  color: #000;
  border: none;
  border-radius: var(--radius-sm);
  font-size: 12px;
  cursor: pointer;
  font-weight: 500;
}

.goto-btn:hover {
  background: var(--accent-hover);
}

/* ============ Settings View ============ */
.settings-view {
  height: 100vh;
  overflow-y: auto;
  padding: 24px 28px;
}

.settings-header {
  margin-bottom: 24px;
}

.settings-header h2 {
  font-size: 20px;
  font-weight: 600;
  display: flex;
  align-items: center;
  gap: 10px;
}

.settings-grid {
  display: flex;
  flex-direction: column;
  gap: 16px;
  max-width: 680px;
}

.settings-card {
  background: var(--bg-surface);
  border: 1px solid var(--border);
  border-radius: var(--radius-lg);
  overflow: hidden;
}

.card-header {
  padding: 16px 20px;
  border-bottom: 1px solid var(--border);
  display: flex;
  align-items: center;
  gap: 10px;
  background: rgba(255, 255, 255, 0.02);
}

.card-header .card-icon {
  width: 20px;
  height: 20px;
  color: var(--gold);
  flex-shrink: 0;
}

.card-header h3 {
  font-size: 14px;
  font-weight: 600;
}

.card-body {
  padding: 20px;
}

/* License Flow */
.license-flow {
  display: flex;
  flex-direction: column;
  gap: 16px;
  margin-bottom: 20px;
}

.flow-step {
  display: flex;
  gap: 14px;
  align-items: flex-start;
}

.step-num {
  width: 26px;
  height: 26px;
  border-radius: 50%;
  background: rgba(255, 215, 0, 0.1);
  border: 1px solid rgba(255, 215, 0, 0.2);
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 12px;
  font-weight: 600;
  color: var(--gold);
  flex-shrink: 0;
}

.step-content {
  display: flex;
  flex-direction: column;
  gap: 6px;
  flex: 1;
  min-width: 0;
}

.step-label {
  font-size: 13px;
  font-weight: 500;
}

.step-hint {
  font-size: 12px;
  color: var(--gold);
}

.machine-code-box {
  display: flex;
  align-items: center;
  gap: 8px;
  background: var(--bg-deep);
  border: 1px solid var(--border);
  border-radius: var(--radius-md);
  padding: 8px 12px;
}

.machine-code-box code {
  flex: 1;
  font-family: var(--font-mono);
  font-size: 12px;
  word-break: break-all;
  color: var(--gold-dim);
}

.icon-btn {
  width: 28px;
  height: 28px;
  border-radius: var(--radius-sm);
  border: 1px solid var(--border);
  background: var(--bg-card);
  color: var(--text-secondary);
  cursor: pointer;
  display: flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
  transition: all 0.15s;
}

.icon-btn:hover {
  background: var(--bg-hover);
  color: var(--gold);
  border-color: var(--gold-dim);
}

.icon-btn svg {
  width: 14px;
  height: 14px;
}

.code-input {
  width: 100%;
  background: var(--bg-deep);
  border: 1px solid var(--border);
  border-radius: var(--radius-md);
  padding: 10px 12px;
  color: var(--text-primary);
  font-size: 13px;
  font-family: var(--font-mono);
  outline: none;
  transition: border-color 0.15s;
}

.code-input:focus {
  border-color: var(--gold-dim);
}

.code-input::placeholder {
  color: var(--text-muted);
}

.activation-msg {
  margin-top: 12px;
  padding: 10px 14px;
  border-radius: var(--radius-md);
  font-size: 13px;
  line-height: 1.5;
}

.activation-msg.success {
  background: var(--success-bg);
  color: var(--success);
  border: 1px solid rgba(74, 222, 128, 0.2);
}

.activation-msg.error {
  background: var(--error-bg);
  color: var(--error);
  border: 1px solid rgba(248, 113, 113, 0.2);
}

/* License Info */
.license-info-list {
  display: flex;
  flex-direction: column;
  gap: 0;
  margin-bottom: 16px;
}

.info-item {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 10px 0;
  border-bottom: 1px solid var(--border);
}

.info-item:last-child {
  border-bottom: none;
}

.info-label {
  font-size: 13px;
  color: var(--text-secondary);
}

.info-value {
  font-size: 13px;
  font-weight: 500;
}

.info-value.warn {
  color: var(--warn);
}

.info-value.mono {
  font-family: var(--font-mono);
  font-size: 11px;
  color: var(--text-muted);
}

.activated-text {
  display: flex;
  align-items: center;
  gap: 6px;
  color: var(--success);
}

.active-dot {
  width: 6px;
  height: 6px;
  border-radius: 50%;
  background: var(--success);
  box-shadow: 0 0 4px rgba(74, 222, 128, 0.5);
}

/* Form */
.form-group {
  margin-bottom: 14px;
}

.form-group label {
  display: block;
  font-size: 12px;
  color: var(--text-secondary);
  margin-bottom: 6px;
  font-weight: 500;
}

.form-group input,
.form-group select {
  width: 100%;
  background: var(--bg-deep);
  border: 1px solid var(--border);
  border-radius: var(--radius-md);
  padding: 10px 12px;
  color: var(--text-primary);
  font-size: 13px;
  font-family: inherit;
  outline: none;
  transition: border-color 0.15s;
}

.form-group input:focus,
.form-group select:focus {
  border-color: var(--gold-dim);
}

.form-group select {
  cursor: pointer;
  appearance: none;
  background-image: url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='12' height='12' viewBox='0 0 24 24' fill='none' stroke='%23a09880' stroke-width='2'%3E%3Cpolyline points='6 9 12 15 18 9'%3E%3C/polyline%3E%3C/svg%3E");
  background-repeat: no-repeat;
  background-position: right 12px center;
  padding-right: 36px;
}

.form-hint {
  display: block;
  font-size: 11px;
  color: var(--text-muted);
  margin-top: 4px;
}

.input-with-toggle {
  position: relative;
}

.input-with-toggle input {
  width: 100%;
  padding-right: 40px;
}

.toggle-btn {
  position: absolute;
  right: 4px;
  top: 50%;
  transform: translateY(-50%);
  width: 32px;
  height: 32px;
  border-radius: var(--radius-sm);
  border: none;
  background: transparent;
  color: var(--text-muted);
  cursor: pointer;
  display: flex;
  align-items: center;
  justify-content: center;
  transition: color 0.15s;
}

.toggle-btn:hover {
  color: var(--text-primary);
}

.toggle-btn svg {
  width: 16px;
  height: 16px;
}

/* Buttons */
.btn {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  padding: 8px 18px;
  border-radius: var(--radius-md);
  font-size: 13px;
  font-weight: 500;
  cursor: pointer;
  transition: all 0.15s;
  border: none;
  font-family: inherit;
}

.btn svg {
  width: 16px;
  height: 16px;
}

.btn-primary {
  background: var(--gold);
  color: #000;
}

.btn-primary:hover:not(:disabled) {
  background: var(--accent-hover);
  box-shadow: 0 0 12px var(--gold-glow);
}

.btn-primary:disabled {
  opacity: 0.4;
  cursor: not-allowed;
}

.btn-secondary {
  background: var(--bg-card);
  color: var(--text-primary);
  border: 1px solid var(--border);
}

.btn-secondary:hover:not(:disabled) {
  background: var(--bg-hover);
  border-color: var(--border-light);
}

.btn-secondary:disabled {
  opacity: 0.4;
  cursor: not-allowed;
}

.btn-danger {
  background: transparent;
  color: var(--error);
  border: 1px solid var(--error);
}

.btn-danger:hover {
  background: var(--error-bg);
}

.btn-spinner {
  width: 14px;
  height: 14px;
  border: 2px solid rgba(0, 0, 0, 0.2);
  border-top-color: #000;
  border-radius: 50%;
  animation: spin 0.6s linear infinite;
}

@keyframes spin {
  to { transform: rotate(360deg); }
}

.btn-row {
  display: flex;
  gap: 8px;
}

/* Environment Check */
.env-check-list {
  display: flex;
  flex-direction: column;
  gap: 0;
  margin-bottom: 16px;
  border: 1px solid var(--border);
  border-radius: var(--radius-md);
  overflow: hidden;
}

.env-item {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 10px 14px;
  border-bottom: 1px solid var(--border);
  font-size: 12px;
}

.env-item:last-child {
  border-bottom: none;
}

.env-check-label {
  width: 100px;
  color: var(--text-secondary);
  font-weight: 500;
  flex-shrink: 0;
}

.env-check-status {
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: 12px;
  width: 70px;
  flex-shrink: 0;
}

.env-check-status.ok {
  color: var(--success);
}

.env-check-status.fail {
  color: var(--error);
}

.check-dot {
  width: 5px;
  height: 5px;
  border-radius: 50%;
  background: currentColor;
}

.env-check-path {
  color: var(--text-muted);
  font-family: var(--font-mono);
  font-size: 11px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.env-hint {
  color: var(--warn);
  font-size: 11px;
  flex: 1;
}

.env-actions {
  display: flex;
  gap: 8px;
  margin-bottom: 12px;
}

.btn-sm {
  padding: 4px 10px;
  font-size: 11px;
  border-radius: var(--radius-sm);
}

.btn-outline {
  background: transparent;
  border: 1px solid var(--border);
  color: var(--text-secondary);
  cursor: pointer;
  text-decoration: none;
  display: inline-flex;
  align-items: center;
  gap: 4px;
  transition: all 0.2s;
}

.btn-outline:hover {
  border-color: var(--accent);
  color: var(--accent);
}

.env-overall {
  padding: 10px 14px;
  border-radius: var(--radius-md);
  font-size: 12px;
  display: flex;
  align-items: center;
  gap: 8px;
  margin-bottom: 12px;
}

.env-overall.ready {
  background: var(--success-bg);
  color: var(--success);
  border: 1px solid rgba(74, 222, 128, 0.2);
}

.env-overall.not-ready {
  background: var(--error-bg);
  color: var(--error);
  border: 1px solid rgba(248, 113, 113, 0.2);
}

.overall-dot {
  width: 6px;
  height: 6px;
  border-radius: 50%;
  background: currentColor;
  flex-shrink: 0;
}

/* ============ Iframe View ============ */
.iframe-view {
  width: 100%;
  height: 100vh;
  overflow: hidden;
}

.app-iframe {
  width: 100%;
  height: 100%;
  border: none;
  background: #fff;
}

/* ============ Guide Page ============ */
.guide-grid {
  max-width: 700px;
}

.guide-text {
  font-size: 14px;
  line-height: 1.7;
  color: var(--text-primary);
}

.guide-flow {
  display: flex;
  flex-direction: column;
  gap: 16px;
}

.guide-step {
  display: flex;
  align-items: flex-start;
  gap: 14px;
}

.guide-step-num {
  width: 28px;
  height: 28px;
  border-radius: 50%;
  background: rgba(255, 215, 0, 0.1);
  border: 1px solid rgba(255, 215, 0, 0.2);
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 13px;
  font-weight: 600;
  color: var(--gold);
  flex-shrink: 0;
}

.guide-step strong {
  display: block;
  font-size: 14px;
  margin-bottom: 4px;
  color: var(--text-primary);
}

.guide-hint {
  font-size: 13px;
  color: var(--text-secondary);
  line-height: 1.5;
}

.guide-list {
  list-style: none;
  padding: 0;
  display: flex;
  flex-direction: column;
  gap: 10px;
}

.guide-list li {
  font-size: 13px;
  line-height: 1.6;
  color: var(--text-secondary);
  padding-left: 16px;
  position: relative;
}

.guide-list li::before {
  content: '•';
  position: absolute;
  left: 0;
  color: var(--gold);
}

.guide-list li strong {
  color: var(--text-primary);
}

.guide-contact {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-top: 12px;
  padding: 12px 16px;
  background: var(--bg-card);
  border: 1px solid var(--border);
  border-radius: var(--radius-md);
}

.contact-icon {
  font-size: 20px;
  line-height: 1;
}

.contact-label {
  font-size: 14px;
  color: var(--text-secondary);
}

.contact-value {
  font-size: 14px;
  font-weight: 600;
  color: var(--gold);
}

.launch-btn {
  font-size: 15px;
  padding: 12px 32px;
}
</style>
