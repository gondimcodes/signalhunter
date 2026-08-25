# Installation & Deployment Guide — SignalHunter
## Target Operating System: Debian 13 (Trixie) / Enterprise Linux

This document outlines the step-by-step procedure to install **Rust**, compile the source code, and deploy **SignalHunter** on a production server running **Debian 13 (Trixie)** (or compatible Linux distributions) using **MariaDB/MySQL**, **HTTPS/TLS** encryption, and a hardened **systemd** service.

---

## 1. Rust Installation and Project Compilation

If compiling directly on the server or on your Linux development machine:

### 1.1. Install System Build Dependencies

Install the C/C++ toolchain, OpenSSL development headers, `pkg-config`, `git`, and essential utilities:

```bash
sudo apt update && sudo apt install -y \
    build-essential \
    pkg-config \
    libssl-dev \
    git \
    curl
```

### 1.2. Install Rust (Official rustup)

Install the stable Rust toolchain:

```bash
# Download and install Rust via rustup (non-interactive default)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y

# Source environment variables in current shell session
source "$HOME/.cargo/env"

# Verify installation
rustc --version
cargo --version
```

### 1.3. Compile SignalHunter in Release Mode

Navigate to the project root directory and build the optimized production binary:

```bash
cd /path/to/signalhunter

# Build a static, universal binary (runs across any Linux distribution)
cargo build --release --target x86_64-unknown-linux-musl
```

The standalone, zero-dependency release binary is produced at:
```text
target/x86_64-unknown-linux-musl/release/signalhunter
```

---

## 2. Server Prerequisites and Packages

Install runtime dependencies on Debian 13 (MariaDB Server, networking tools, OpenSSL, and administrative utilities):

```bash
sudo apt update && sudo apt install -y \
    mariadb-server \
    mariadb-client \
    openssl \
    ca-certificates \
    snmp \
    curl \
    ufw
```

---

## 3. Directory Structure & Service User Setup

For system isolation and security hardening, SignalHunter runs under a dedicated, unprivileged system account:

```bash
# 1. Create isolated system group and user
sudo adduser --system --group --no-create-home --shell /bin/false signalhunter

# 2. Create production directories
sudo mkdir -p /opt/signalhunter/certs
sudo mkdir -p /opt/signalhunter/reports
sudo mkdir -p /var/log/signalhunter

# 3. Apply ownership and strict file permissions
sudo chown -R signalhunter:signalhunter /opt/signalhunter /var/log/signalhunter
sudo chmod 750 /opt/signalhunter
sudo chmod 700 /opt/signalhunter/certs
```

---

## 4. Database Setup (MariaDB / MySQL)

Run the initial MariaDB security script:

```bash
sudo mysql_secure_installation
```

Log in to MariaDB and provision the dedicated database and user credentials:

```bash
sudo mariadb
```

Execute the following SQL statements (substitute `'YourStrongSecretPassword123!'` with your actual password):

```sql
-- Create database with utf8mb4 encoding
CREATE DATABASE signalhunter CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci;

-- Create local service user
CREATE USER 'signalhunter_user'@'localhost' IDENTIFIED BY 'YourStrongSecretPassword123!';

-- Grant privileges
GRANT ALL PRIVILEGES ON signalhunter.* TO 'signalhunter_user'@'localhost';
FLUSH PRIVILEGES;
EXIT;
```

Import the database schema:

```bash
mariadb -u signalhunter_user -p signalhunter < /opt/signalhunter/schema.sql
```

---

## 5. File Deployment to Target Server

Deploy release artifacts from your build machine to `/opt/signalhunter`:

```bash
# Example scp command from development host:
scp target/x86_64-unknown-linux-musl/release/signalhunter config.toml schema.sql user@server-ip:/tmp/

# On the server host, move files into place:
sudo mv /tmp/signalhunter /opt/signalhunter/
sudo mv /tmp/config.toml /opt/signalhunter/
sudo mv /tmp/schema.sql /opt/signalhunter/

# Grant executable permissions to binary
sudo chmod +x /opt/signalhunter/signalhunter

# Restrict config.toml permissions (contains AES keys and DB credentials)
sudo chmod 600 /opt/signalhunter/config.toml

# Set proper ownership
sudo chown -R signalhunter:signalhunter /opt/signalhunter
```

---

## 6. Security Configuration (`config.toml`)

Edit `/opt/signalhunter/config.toml`:

```bash
sudo nano /opt/signalhunter/config.toml
```

### 6.1. Generate AES-256-GCM Master Encryption Key
To encrypt OLT management credentials and SNMP community strings at rest:

```bash
openssl rand -hex 32
```

### 6.2. Generate JWT Secret Key
To sign user session tokens securely:

```bash
openssl rand -base64 48
```

### 6.3. Sample Production `/opt/signalhunter/config.toml`:

```toml
[server]
host = "0.0.0.0"
port = 8443
use_tls = true
tls_cert_path = "certs/cert.pem"
tls_key_path = "certs/key.pem"

[database]
host = "127.0.0.1"
port = 3306
username = "signalhunter_user"
password = "YourStrongSecretPassword123!"
database = "signalhunter"
max_connections = 25
min_connections = 5
connect_timeout_sec = 10
idle_timeout_sec = 300

[security]
master_encryption_key = "YOUR_64_CHAR_HEX_GENERATED_ABOVE"
jwt_secret = "YOUR_BASE64_STRING_GENERATED_ABOVE"
jwt_expiration_hours = 24

[collector]
default_collection_interval_mins = 1440
max_concurrent_olt_scans = 10
max_concurrent_requests_per_olt = 2
request_timeout_sec = 15
default_protocol = "snmp"

[thresholds]
rx_excellent_min = -18.0
rx_excellent_max = -14.0
rx_good_min = -23.0
rx_good_max = -8.0
rx_warning_min = -26.9
rx_critical_min = -27.0
degradation_alert_delta_db = 3.0
```

---

## 7. TLS / HTTPS Certificate Provisioning

### Option A: Let's Encrypt (Certbot)
```bash
sudo apt install -y certbot
sudo certbot certonly --standalone -d signalhunter.yourdomain.com

# Copy certificates to /opt/signalhunter/certs:
sudo cp /etc/letsencrypt/live/signalhunter.yourdomain.com/fullchain.pem /opt/signalhunter/certs/cert.pem
sudo cp /etc/letsencrypt/live/signalhunter.yourdomain.com/privkey.pem /opt/signalhunter/certs/key.pem
sudo chown -R signalhunter:signalhunter /opt/signalhunter/certs
sudo chmod 600 /opt/signalhunter/certs/*.pem
```

### Option B: Self-Signed Certificate (Internal Lab / Testing)
```bash
sudo openssl req -x509 -nodes -days 365 -newkey rsa:2048 \
    -keyout /opt/signalhunter/certs/key.pem \
    -out /opt/signalhunter/certs/cert.pem \
    -subj "/C=BR/ST=SP/L=SaoPaulo/O=ISP/OU=NOC/CN=signalhunter.local"

sudo chown -R signalhunter:signalhunter /opt/signalhunter/certs
sudo chmod 600 /opt/signalhunter/certs/*.pem
```

---

## 8. Systemd Service Unit Creation

Create the systemd service file:

```bash
sudo nano /etc/systemd/system/signalhunter.service
```

Insert the following unit configuration:

```ini
[Unit]
Description=SignalHunter - Intelligent Optical Telemetry & Diagnostics
After=network.target mariadb.service
Wants=mariadb.service

[Service]
Type=simple
User=signalhunter
Group=signalhunter
WorkingDirectory=/opt/signalhunter
ExecStart=/opt/signalhunter/signalhunter
Restart=always
RestartSec=5s

# Allow binding to privileged ports (< 1024) without root
AmbientCapabilities=CAP_NET_BIND_SERVICE
CapabilityBoundingSet=CAP_NET_BIND_SERVICE

# Kernel Hardening
NoNewPrivileges=true
ProtectSystem=full
ProtectHome=true
PrivateTmp=true
ProtectControlGroups=true
ProtectKernelModules=true

# File Descriptors Limit
LimitNOFILE=65535

# Logs
StandardOutput=append:/var/log/signalhunter/output.log
StandardError=append:/var/log/signalhunter/error.log

[Install]
WantedBy=multi-user.target
```

---

## 9. First Run & Admin Password Generation

> [!IMPORTANT]
> **CRITICAL — AUTOMATIC INITIAL PASSWORD GENERATION:**
> On the very first run against a clean database, SignalHunter detects that no users exist and **automatically generates a secure, random 20-character password** for the `admin` account.
> 
> You **must** observe this startup output to record the generated password and access the web UI!

### 9.1. Bootstrap & Password Retrieval

You have two convenient options for the first run:

#### Option A (Recommended): One-Time Manual Run in Terminal
Watch the schema initialization in real time and copy the credentials directly:

```bash
cd /opt/signalhunter
sudo -u signalhunter ./signalhunter
```

You will see the startup banner:
```text
[INFO  signalhunter] Conexão com o banco de dados MySQL estabelecida com sucesso!
[INFO  signalhunter] Bootstrap inicial do schema concluído.

=====================================================
 CREDENCIAIS DE ACESSO INICIAL (PRIMEIRA EXECUÇÃO)
 Usuário : admin
 Senha   : aB3xK9vL2mP8qR5wT1zY
 Altere a senha imediatamente após o primeiro login!
=====================================================
```
> After copying the password, press `Ctrl + C` to exit and start the systemd service below.

---

#### Option B: Start Directly via Systemd Service
The password will be recorded in `/var/log/signalhunter/output.log` and the systemd journal:

```bash
# 1. Reload systemd daemon
sudo systemctl daemon-reload

# 2. Enable service on system boot
sudo systemctl enable signalhunter

# 3. Start SignalHunter service
sudo systemctl start signalhunter

# 4. View generated password from log file:
sudo cat /var/log/signalhunter/output.log

# (Or via journalctl):
sudo journalctl -u signalhunter -n 50 --no-pager
```

> 💡 **Store this password safely!** It is only generated on initial database bootstrap. Once logged into the web interface, you can modify it under the **Users** tab.

---

## 10. Firewall Configuration (UFW)

Allow access to the configured web port (e.g., `8443`):

```bash
sudo ufw allow 8443/tcp comment 'SignalHunter Web UI'
sudo ufw enable
sudo ufw status
```

> **Network Routing Requirements:**  
> Ensure the server has IP routing to all OLTs on:
> - **SNMP:** UDP `161` (100% Universal SNMPv2c Telemetry)

---

## 11. Production Hardening Checklist

Before releasing to NOC operations:
- [x] Initial run performed and `admin` generated password captured.
- [x] `/opt/signalhunter/config.toml` permissions restricted to `600` owned by `signalhunter:signalhunter`.
- [x] Master key `master_encryption_key` generated via `openssl rand -hex 32`.
- [x] JWT secret `jwt_secret` generated via `openssl rand -base64 48`.
- [x] TLS certificates placed in `/opt/signalhunter/certs/` with `600` permissions.
- [x] Service running as unprivileged `signalhunter` account with systemd kernel protections.

---

## 12. Application Access

Open your web browser and navigate to:
- **URL:** `https://server-ip:8443`
- **Username:** `admin`
- **Password:** *(The random 20-character password captured during the first execution)*
