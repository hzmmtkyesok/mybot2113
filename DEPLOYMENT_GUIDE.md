# Polymarket Copy Trading Bot 部署指南

## 🔧 修复的问题

### 1. OpenSSL 依赖错误（已修复）
**错误信息**: `openssl-sys@0.9.111: Could not find directory of OpenSSL installation`

**解决方案**: 修改 `Cargo.toml`，将所有依赖改为使用 `rustls` 替代 OpenSSL：
- `reqwest`: 使用 `rustls-tls` feature
- `ethers`: 使用 `rustls` feature
- `tokio-tungstenite`: 使用 `rustls-tls-native-roots` feature
- `tungstenite`: 使用 `rustls-tls-native-roots` feature

### 2. WebSocket 连接错误（已修复）
**错误信息**: `WebSocket error`

**解决方案**:
- 添加了连接超时处理（30秒）
- 添加了自动重连逻辑（指数退避）
- 改进了错误处理和日志记录
- 添加了 URL 验证
- 修复了 ping/pong 心跳机制

### 3. 代码编译错误（已修复）
- `types.rs`: `impl Default for Config` 移到 `Config` 结构体定义之后
- `mempool_monitor.rs`: `is_multiple_of` 改为 `% 2 == 0`

---

## 📋 系统要求

- **操作系统**: Linux (Ubuntu/Debian/CentOS) 或 macOS
- **Rust**: 1.70.0 或更高版本
- **内存**: 最低 512MB RAM
- **网络**: 稳定的互联网连接

---

## 🚀 快速部署步骤

### 步骤 1: 安装 Rust

```bash
# 安装 Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# 加载环境变量
source $HOME/.cargo/env

# 验证安装
rustc --version
cargo --version
```

### 步骤 2: 克隆/解压项目

```bash
# 如果是zip文件
unzip polymarket-copy-botik-main.zip
cd polymarket-copy-botik-main

# 或者克隆
git clone <your-repo-url>
cd polymarket-copy-botik-main
```

### 步骤 3: 配置环境变量

```bash
# 复制示例配置
cp .env.example .env

# 编辑配置文件
nano .env  # 或使用 vim .env
```

**必须配置的变量**:
```env
# 要跟踪的钱包地址（逗号分隔）
WALLETS_TO_TRACK=0x1234567890abcdef...

# 你的钱包地址
YOUR_WALLET=0xYourWalletAddress

# 你的私钥（保密！）
PRIVATE_KEY=0xYourPrivateKey

# RPC URL（必须是 WebSocket URL）
# 推荐使用 Alchemy 或 Infura 的 Polygon 节点
RPC_URL=wss://polygon-mainnet.g.alchemy.com/v2/YOUR_API_KEY
```

### 步骤 4: 构建项目

```bash
# 赋予执行权限
chmod +x build.sh

# 构建
./build.sh

# 或者直接用 cargo
cargo build --release
```

### 步骤 5: 运行机器人

```bash
# 运行主程序
./target/release/polymarket-bot

# 或使用 cargo
cargo run --release --bin polymarket-bot

# 设置日志级别
RUST_LOG=info ./target/release/polymarket-bot
RUST_LOG=debug ./target/release/polymarket-bot  # 更详细的日志
```

---

## 🐳 Docker 部署（推荐）

### Dockerfile

创建 `Dockerfile`:

```dockerfile
FROM rust:1.75-slim as builder

WORKDIR /app
COPY . .

# 构建
RUN cargo build --release

# 运行时镜像
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY --from=builder /app/target/release/polymarket-bot .
COPY --from=builder /app/.env.example .env

CMD ["./polymarket-bot"]
```

### Docker Compose

创建 `docker-compose.yml`:

```yaml
version: '3.8'

services:
  polymarket-bot:
    build: .
    restart: unless-stopped
    env_file:
      - .env
    environment:
      - RUST_LOG=info
    volumes:
      - ./logs:/app/logs
```

### 运行 Docker

```bash
# 构建并运行
docker-compose up -d

# 查看日志
docker-compose logs -f

# 停止
docker-compose down
```

---

## 🖥️ systemd 服务部署（Linux）

### 创建服务文件

```bash
sudo nano /etc/systemd/system/polymarket-bot.service
```

内容：
```ini
[Unit]
Description=Polymarket Copy Trading Bot
After=network.target

[Service]
Type=simple
User=your-username
WorkingDirectory=/path/to/polymarket-copy-botik-main
Environment=RUST_LOG=info
ExecStart=/path/to/polymarket-copy-botik-main/target/release/polymarket-bot
Restart=always
RestartSec=10

[Install]
WantedBy=multi-user.target
```

### 启用并运行服务

```bash
# 重载服务配置
sudo systemctl daemon-reload

# 启用开机自启
sudo systemctl enable polymarket-bot

# 启动服务
sudo systemctl start polymarket-bot

# 查看状态
sudo systemctl status polymarket-bot

# 查看日志
journalctl -u polymarket-bot -f
```

---

## ☁️ 云服务器部署

### AWS EC2 / 阿里云 ECS

1. 选择 Ubuntu 22.04 LTS 镜像
2. 实例类型：t3.small 或以上（1核 2GB 内存）
3. 安全组：出站允许所有，入站仅允许 SSH (22)

```bash
# 连接服务器
ssh -i your-key.pem ubuntu@your-server-ip

# 更新系统
sudo apt update && sudo apt upgrade -y

# 安装必要工具
sudo apt install -y build-essential pkg-config

# 按上述步骤安装 Rust 和部署
```

### 使用 Screen 后台运行

```bash
# 安装 screen
sudo apt install screen

# 创建新会话
screen -S polymarket-bot

# 运行程序
./target/release/polymarket-bot

# 分离会话 (Ctrl+A, D)

# 重新连接
screen -r polymarket-bot
```

---

## 🔍 故障排除

### WebSocket 连接失败

1. **检查 RPC URL**:
   - 确保使用 `wss://` 开头的 WebSocket URL
   - 确保 API Key 正确

2. **检查网络**:
   ```bash
   curl -I https://api.polymarket.com
   ```

3. **增加日志级别**:
   ```bash
   RUST_LOG=debug ./target/release/polymarket-bot
   ```

### 编译错误

1. **更新 Rust**:
   ```bash
   rustup update
   ```

2. **清理并重建**:
   ```bash
   cargo clean
   cargo build --release
   ```

### 权限错误

```bash
chmod +x build.sh
chmod +x target/release/polymarket-bot
```

---

## 📊 监控和日志

### 日志级别

```bash
# 仅错误
RUST_LOG=error ./target/release/polymarket-bot

# 警告和错误
RUST_LOG=warn ./target/release/polymarket-bot

# 信息级别（推荐）
RUST_LOG=info ./target/release/polymarket-bot

# 调试级别
RUST_LOG=debug ./target/release/polymarket-bot

# 特定模块日志
RUST_LOG=polymarket_copy_bot::watcher=debug ./target/release/polymarket-bot
```

### 输出到文件

```bash
./target/release/polymarket-bot 2>&1 | tee -a bot.log
```

---

## ⚠️ 安全提示

1. **保护私钥**: 永远不要将 `.env` 文件提交到 Git
2. **使用环境变量**: 生产环境使用环境变量而非 `.env` 文件
3. **限制权限**: `chmod 600 .env`
4. **防火墙**: 仅开放必要端口
5. **定期更新**: 保持系统和依赖更新

---

## 📞 支持

如有问题，请检查：
1. 日志输出
2. 环境变量配置
3. 网络连接
4. RPC 服务状态
