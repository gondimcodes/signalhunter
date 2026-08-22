# Guia de Instalação e Implantação — SignalHunter
## Sistema Operacional Alvo: Debian 13 (Trixie)

Este documento descreve o procedimento passo a passo para instalar o **Rust**, compilar o código-fonte e implantar o **SignalHunter** em um servidor executando **Debian 13 (Trixie)**, utilizando banco de dados **MariaDB/MySQL**, suporte a **HTTPS/TLS** e serviço gerenciado via **systemd**.

---

## 1. Instalação do Rust e Compilação do Projeto

Caso você vá compilar o projeto diretamente no servidor ou em sua máquina de desenvolvimento Linux/Debian:

### 1.1. Instalar Dependências de Compilação do Sistema

Instale o compilador C/C++, OpenSSL de desenvolvimento, `pkg-config`, `git` e ferramentas essenciais:

```bash
sudo apt update && sudo apt install -y \
    build-essential \
    pkg-config \
    libssl-dev \
    git \
    curl
```

### 1.2. Instalar o Rust (via rustup oficial)

Instale a versão estável do Rust utilizando o instalador oficial `rustup`:

```bash
# Baixar e executar o instalador do rustup (modo padrão/default)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y

# Carregar as variáveis de ambiente do Cargo na sessão atual
source "$HOME/.cargo/env"

# Verificar a instalação
rustc --version
cargo --version
```

### 1.3. Clonar e Compilar o SignalHunter em Modo Release

Navegue até o diretório onde o código-fonte foi clonado ou descompactado e execute a compilação otimizada para produção:

```bash
# Acessar o diretório do código-fonte do projeto
cd /caminho/para/signalhunter

# Compilar o binário estático universal (roda em qualquer Linux: Debian 11, 12, 13, Ubuntu, CentOS, etc.)
cargo build --release --target x86_64-unknown-linux-musl
```

O binário final autônomo (100% estático e sem dependências) será gerado em:
```text
target/x86_64-unknown-linux-musl/release/signalhunter
```

---

## 2. Requisitos do Servidor e Dependências

Instale os pacotes necessários no Debian 13 (Servidor MariaDB, utilitários de rede, OpenSSL e ferramentas de administração):

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

## 3. Criação da Estrutura de Diretórios e Usuário de Serviço

Por questões de segurança, o daemon do SignalHunter deve rodar sob um usuário de sistema dedicado e sem privilégios de root:

```bash
# 1. Criar usuário e grupo de sistema
sudo adduser --system --group --no-create-home --shell /bin/false signalhunter

# 2. Criar diretórios de instalação, certificados e relatórios
sudo mkdir -p /opt/signalhunter/certs
sudo mkdir -p /opt/signalhunter/reports
sudo mkdir -p /var/log/signalhunter

# 3. Ajustar permissões
sudo chown -R signalhunter:signalhunter /opt/signalhunter /var/log/signalhunter
sudo chmod 750 /opt/signalhunter
sudo chmod 700 /opt/signalhunter/certs
```

---

## 4. Configuração do Banco de Dados (MariaDB / MySQL)

Execute o assistente de segurança inicial do MariaDB:

```bash
sudo mysql_secure_installation
```

Em seguida, crie a base de dados, usuário e aplique o esquema relacional:

```bash
# Acessar o console do MariaDB como root
sudo mariadb
```

Dentro do console MariaDB, execute os seguintes comandos SQL (substitua `'SuaSenhaForteAqui123!'` pela sua senha real):

```sql
-- Criar a base de dados
CREATE DATABASE signalhunter CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci;

-- Criar usuário com acesso local
CREATE USER 'signalhunter_user'@'localhost' IDENTIFIED BY 'SuaSenhaForteAqui123!';

-- Conceder permissões
GRANT ALL PRIVILEGES ON signalhunter.* TO 'signalhunter_user'@'localhost';
FLUSH PRIVILEGES;
EXIT;
```

Importe o arquivo `schema.sql`:

```bash
# Copie o schema.sql para o servidor e importe:
mariadb -u signalhunter_user -p signalhunter < /opt/signalhunter/schema.sql
```

---

## 5. Transferência dos Arquivos para o Servidor

Copie os arquivos do ambiente de desenvolvimento para o servidor remoto:

```bash
# Exemplo de comando executado a partir da sua máquina de desenvolvimento:
scp target/x86_64-unknown-linux-musl/release/signalhunter config.toml schema.sql logo.png usuario@ip-do-servidor:/tmp/

# No servidor remoto, mova para /opt/signalhunter:
sudo mv /tmp/signalhunter /opt/signalhunter/
sudo mv /tmp/config.toml /opt/signalhunter/
sudo mv /tmp/schema.sql /opt/signalhunter/
sudo mv /tmp/logo.png /opt/signalhunter/

# Dar permissão de execução ao binário
sudo chmod +x /opt/signalhunter/signalhunter

# Hardening e Proteção de Credenciais Sensíveis:
# O config.toml contém senhas de banco, JWT secret e chaves AES. 
# Deve ter permissão estrita 600 (leitura/escrita exclusiva do usuário do serviço)
sudo chmod 600 /opt/signalhunter/config.toml

# Ajustar propriedade de todos os arquivos para o usuário isolado
sudo chown -R signalhunter:signalhunter /opt/signalhunter
```

---

## 6. Configuração de Segurança e Arquivo `config.toml`

Edite o arquivo `/opt/signalhunter/config.toml`:

```bash
sudo nano /opt/signalhunter/config.toml
```

### 6.1. Gerar Chave Mestra de Criptografia AES-256-GCM
Para cifrar senhas de OLTs e credenciais SNMP no banco de dados, gere uma chave criptográfica forte de 32 bytes (64 caracteres hex):

```bash
openssl rand -hex 32
```
### 6.2. Gerar Chave Secreta para Sessões JWT
Para autenticar os usuários no frontend via token JWT com segurança criptográfica, gere uma chave pseudo-aleatória forte de alta entropia (64 bytes em Base64 ou Hexadecimal):

```bash
# Gerar chave aleatória forte de 64 bytes codificada em Base64:
openssl rand -base64 48
```
*Copie a string gerada e cole no campo `jwt_secret` do arquivo `/opt/signalhunter/config.toml`.*

Você também pode configurar o tempo de expiração do token de sessão de acordo com a política de segurança da sua empresa:
- `jwt_expiration_hours = 24` (padrão: sessão válida por 24 horas).
- Para maior segurança em ambientes corporativos sensíveis, você pode reduzir para `8` ou `12` horas.

### 6.3. Exemplo de `/opt/signalhunter/config.toml` ajustado para produção:

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
password = "SuaSenhaForteAqui123!"
database = "signalhunter"
max_connections = 25
min_connections = 5
connect_timeout_sec = 10
idle_timeout_sec = 300

[security]
master_encryption_key = "SEU_HEX_DE_64_CARACTERES_GERADO_ACIMA"
jwt_secret = "SEU_BASE64_GERADO_ACIMA"
jwt_expiration_hours = 24

[collector]
default_collection_interval_mins = 60
max_concurrent_olt_scans = 10
max_concurrent_requests_per_olt = 2
pon_inter_scan_delay_ms = 50
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

## 7. Configuração dos Certificados TLS / HTTPS

### Opção A: Certificado SSL Válido (Let's Encrypt / Certbot)
```bash
sudo apt install -y certbot
sudo certbot certonly --standalone -d signalhunter.seudominio.com.br

# Criar links simbólicos ou copiar para /opt/signalhunter/certs:
sudo cp /etc/letsencrypt/live/signalhunter.seudominio.com.br/fullchain.pem /opt/signalhunter/certs/cert.pem
sudo cp /etc/letsencrypt/live/signalhunter.seudominio.com.br/privkey.pem /opt/signalhunter/certs/key.pem
sudo chown -R signalhunter:signalhunter /opt/signalhunter/certs
sudo chmod 600 /opt/signalhunter/certs/*.pem
```

### Opção B: Certificado Autoassinado (Para Testes / Rede Interna)
```bash
sudo openssl req -x509 -nodes -days 365 -newkey rsa:2048 \
    -keyout /opt/signalhunter/certs/key.pem \
    -out /opt/signalhunter/certs/cert.pem \
    -subj "/C=BR/ST=SP/L=SaoPaulo/O=ISP/OU=NOC/CN=signalhunter.local"

sudo chown -R signalhunter:signalhunter /opt/signalhunter/certs
sudo chmod 600 /opt/signalhunter/certs/*.pem
```

---

## 8. Criação do Serviço no Systemd

Crie a unidade de serviço para gerenciamento automático pelo Debian:

```bash
sudo nano /etc/systemd/system/signalhunter.service
```

Cole o seguinte conteúdo:

```ini
[Unit]
Description=SignalHunter - Sistema Inteligente de Coleta & Diagnóstico Óptico
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

# Permissão para rodar em portas baixas / privilegiadas (< 1024, ex: 80, 443) sem root
AmbientCapabilities=CAP_NET_BIND_SERVICE
CapabilityBoundingSet=CAP_NET_BIND_SERVICE

# Hardening de Segurança no Linux
NoNewPrivileges=true
ProtectSystem=full
ProtectHome=true
PrivateTmp=true
ProtectControlGroups=true
ProtectKernelModules=true

# Limites de Descritores de Arquivos e Conexões
LimitNOFILE=65535

# Logs
StandardOutput=append:/var/log/signalhunter/output.log
StandardError=append:/var/log/signalhunter/error.log

[Install]
WantedBy=multi-user.target
```

---

## 9. Primeira Execução e Geração da Senha do Administrador

> [!IMPORTANT]
> **IMPORTANTE — GERAÇÃO DA SENHA DO USUÁRIO ADMINISTRADOR:**
> Na primeira vez em que o **SignalHunter** é iniciado em um banco de dados novo, ele detecta que não existem usuários e **gera automaticamente uma senha aleatória forte de 20 caracteres** para o usuário `admin`. 
> 
> Você **deve** visualizar os logs dessa primeira inicialização para anotar a senha gerada e conseguir realizar o primeiro login no sistema!

### 9.1. Primeira Execução e Captura da Senha

Você tem duas formas práticas e seguras de executar pela primeira vez:

#### Opção A (Recomendada): Executar uma única vez manualmente no terminal
Desta forma você acompanha o bootstrap completo na tela do terminal e visualiza a senha gerada imediatamente:

```bash
# Acessar a pasta da aplicação
cd /opt/signalhunter

# Executar temporariamente como usuário signalhunter
sudo -u signalhunter ./signalhunter
```

Você verá na tela o banner de inicialização:
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
> Após copiar a senha, pressione `Ctrl + C` para encerrar e inicie o serviço definitivo via **Systemd** abaixo.

---

#### Opção B: Iniciar direto como Serviço Systemd
Se preferir iniciar direto pelo systemd, a senha ficará gravada no log de saída `/var/log/signalhunter/output.log` e no `journalctl`:

```bash
# 1. Recarregar configurações do systemd
sudo systemctl daemon-reload

# 2. Habilitar para iniciar no boot
sudo systemctl enable signalhunter

# 3. Iniciar o serviço SignalHunter
sudo systemctl start signalhunter

# 4. Ler a senha no arquivo de log output.log:
sudo cat /var/log/signalhunter/output.log

# (Ou via journalctl):
sudo journalctl -u signalhunter -n 50 --no-pager
```

> 💡 **Guarde essa senha com segurança!** Ela é gerada e exibida apenas uma única vez na criação da base de dados. Após realizar o login no painel web, você poderá alterá-la na aba **Usuários**.

---

## 10. Configuração do Firewall (UFW)

Libere a porta web configurada (ex: `8443`) e as portas de comunicação com as OLTs:

```bash
# Liberar porta de acesso à interface web
sudo ufw allow 8443/tcp comment 'SignalHunter Web UI'

# Se o firewall estiver desativado, habilite:
sudo ufw enable
sudo ufw status
```

> **Atenção sobre o Firewall e Rotas para as OLTs:**
> Certifique-se de que o servidor possui rota de rede para as OLTs nas portas:
> - **SNMP:** UDP `161`
> - **SSH:** TCP `22` (Huawei, Datacom e ZTE)

---

## 11. Checklist de Segurança (Hardening de Produção)

Antes de liberar para a equipe de operação, certifique-se de que:
- [x] O serviço foi iniciado pela primeira vez e a senha gerada do `admin` foi capturada nos logs.
- [x] O arquivo `/opt/signalhunter/config.toml` possui permissão estrita `600` (`chmod 600`) e pertence exclusivamente ao usuário `signalhunter:signalhunter`.
- [x] A chave mestra `master_encryption_key` foi gerada via `openssl rand -hex 32` e está protegida.
- [x] O segredo `jwt_secret` foi gerado via `openssl rand -base64 48`.
- [x] Os certificados TLS em `/opt/signalhunter/certs/` possuem permissão `600`.
- [x] O serviço roda sob o usuário sem privilégios `signalhunter` com proteções de kernel ativas no systemd.

---

## 12. Acesso à Aplicação

Abra o navegador e acesse:
- **URL:** `https://ip-do-servidor:8443` (ou a porta configurada no seu `config.toml`)
- **Usuário:** `admin`
- **Senha:** *(A senha aleatória de 20 caracteres capturada no log da primeira execução)*
