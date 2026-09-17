#!/usr/bin/env bash
# =============================================================
# create_deploy.sh — SignalHunter deployment helper
# Repositório: https://github.com/gondimcodes/signalhunter
#
# Autor: Alexandre Jeronimo Correa <ajcorrea@gmail.com>
#
# Uso:
#   ./create_deploy.sh deploy   ← primeira instalação completa
#   ./create_deploy.sh update   ← sincroniza GitHub e reconstrói
# =============================================================
set -euo pipefail

# ---- Configurações globais ----
REPO_URL="https://github.com/gondimcodes/signalhunter.git"
BRANCH="main"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WORK_DIR="$SCRIPT_DIR"
REPO_DIR="$WORK_DIR/signalhunter"
DEPLOY_DIR="$WORK_DIR/deploy"

# ---- Cores / log helpers ----
RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[1;33m'
CYAN='\033[0;36m'; BOLD='\033[1m'; NC='\033[0m'

log()  { echo -e "${CYAN}[INFO]${NC}  $*"; }
ok()   { echo -e "${GREEN}[ OK ]${NC}  $*"; }
warn() { echo -e "${YELLOW}[AVISO]${NC} $*"; }
die()  { echo -e "${RED}[ERRO]${NC}  $*" >&2; exit 1; }

# ============================================================
# Verificar dependências
# ============================================================
check_deps() {
    local missing=()
    for cmd in git docker openssl; do
        command -v "$cmd" &>/dev/null || missing+=("$cmd")
    done
    [[ ${#missing[@]} -gt 0 ]] && die "Dependências ausentes: ${missing[*]}"

    docker info &>/dev/null || die \
        "Docker daemon inacessível. Adicione seu usuário ao grupo docker e relogue:\n" \
        "  sudo usermod -aG docker \$USER && newgrp docker"

    ok "Dependências verificadas: git, docker, openssl"
}

# ============================================================
# Gerar segredos
# ============================================================
gen_secrets() {
    DB_PASSWORD="$(openssl rand -hex 16)"
    DB_ROOT_PASSWORD="$(openssl rand -hex 16)"
    MASTER_KEY="$(openssl rand -hex 32)"
    JWT_SECRET="$(openssl rand -hex 48)"
    ok "Segredos gerados com openssl rand"
}

# ============================================================
# Salvar segredos em arquivo protegido
# ============================================================
save_secrets() {
    local f="$DEPLOY_DIR/secrets.txt"
    cat > "$f" << SECRETS_EOF
# ============================================================
# SignalHunter — Segredos gerados em $(date '+%Y-%m-%d %H:%M:%S')
# GUARDE ESTE ARQUIVO EM LOCAL SEGURO. NÃO COMPARTILHE!
# ============================================================

DB_PASSWORD      = ${DB_PASSWORD}
DB_ROOT_PASSWORD = ${DB_ROOT_PASSWORD}
MASTER_KEY       = ${MASTER_KEY}
JWT_SECRET       = ${JWT_SECRET}

# Acesso à aplicação (após subir os containers):
# URL: http://<ip-do-servidor>:${APP_PORT}
# Usuário: admin
# Senha: verifique os logs com: docker compose -f deploy/compose.yaml logs signalhunter
SECRETS_EOF
    chmod 600 "$f"
    ok "Segredos salvos em: $f (permissões 600)"
}

# ============================================================
# Dockerfile (multi-stage: build Rust → runtime mínimo)
# ============================================================
write_dockerfile() {
    cat > "$DEPLOY_DIR/Dockerfile" << 'DOCKERFILE_EOF'
# ============================================================
# Stage 1 — Build  (Rust + musl target para binário estático)
# ============================================================
FROM rust:slim AS builder

RUN apt-get update && apt-get install -y --no-install-recommends \
        pkg-config \
        libssl-dev \
        musl-tools \
    && rm -rf /var/lib/apt/lists/*

RUN rustup target add x86_64-unknown-linux-musl

WORKDIR /build

# Copia o source (inclui vendor/sqlx-mysql — patch local obrigatório)
COPY signalhunter/ .

RUN cargo build --release --target x86_64-unknown-linux-musl

# ============================================================
# Stage 2 — Runtime  (imagem mínima debian bookworm-slim)
# ============================================================
FROM debian:bookworm-slim AS runtime

RUN apt-get update && apt-get install -y --no-install-recommends \
        ca-certificates \
        snmp \
    && rm -rf /var/lib/apt/lists/*

# Usuário isolado com UID/GID 1000 (igual ao usuário do host)
RUN groupadd -g 1000 signalhunter && \
    useradd  -u 1000 -g signalhunter -s /sbin/nologin -M signalhunter

# Criar diretórios de runtime
RUN mkdir -p /opt/signalhunter/reports /etc/signalhunter && \
    chown -R signalhunter:signalhunter /opt/signalhunter /etc/signalhunter && \
    chmod 750 /opt/signalhunter /etc/signalhunter

COPY --from=builder \
    /build/target/x86_64-unknown-linux-musl/release/signalhunter \
    /usr/local/bin/signalhunter

RUN chmod +x /usr/local/bin/signalhunter

USER signalhunter
WORKDIR /opt/signalhunter

EXPOSE ${APP_PORT}

CMD ["/usr/local/bin/signalhunter"]
DOCKERFILE_EOF
    ok "Dockerfile criado"
}

# ============================================================
# .dockerignore  (fica na raiz do build context = $WORK_DIR)
# ============================================================
write_dockerignore() {
    cat > "$WORK_DIR/.dockerignore" << 'IGNORE_EOF'
# Artefatos de build Rust — recompilados dentro do container
signalhunter/target/

# Arquivos de deploy — não fazem parte do source
deploy/

# Git
.git
.gitignore

# Editores / SO
.vscode/
.idea/
*.swp
*.DS_Store
Thumbs.db
IGNORE_EOF
    ok ".dockerignore criado em $WORK_DIR/"
}

# ============================================================
# config.toml  (montado como volume read-only no container)
# ============================================================
write_config() {
    mkdir -p "$DEPLOY_DIR/config"
    cat > "$DEPLOY_DIR/config/config.toml" << CONFIG_EOF
# ============================================================
# SignalHunter — config.toml (Docker / containerizado)
# Gerado automaticamente por create_deploy.sh em $(date '+%Y-%m-%d %H:%M:%S')
# ============================================================
mode = "production"

[server]
host          = "0.0.0.0"
port          = ${APP_PORT}
use_tls       = false
tls_cert_path = "/etc/signalhunter/certs/cert.pem"
tls_key_path  = "/etc/signalhunter/certs/key.pem"

[database]
host                = "mariadb"
port                = 3306
username            = "signalhunter_user"
password            = "${DB_PASSWORD}"
database            = "signalhunter"
max_connections     = 25
min_connections     = 5
connect_timeout_sec = 10
idle_timeout_sec    = 300

[security]
master_encryption_key = "${MASTER_KEY}"
jwt_secret            = "${JWT_SECRET}"
jwt_expiration_hours  = 24

[collector]
default_collection_interval_mins = 1440
max_concurrent_olt_scans         = 10
max_concurrent_requests_per_olt  = 2
request_timeout_sec              = 15
default_protocol                 = "snmp"

[thresholds]
rx_excellent_min           = -18.0
rx_excellent_max           = -14.0
rx_good_min                = -23.0
rx_good_max                = -8.0
rx_warning_min             = -26.9
rx_critical_min            = -27.0
degradation_alert_delta_db = 3.0
CONFIG_EOF
    chmod 600 "$DEPLOY_DIR/config/config.toml"
    ok "config.toml criado (permissões 600)"
}

# ============================================================
# compose.yaml
# ============================================================
write_compose() {
    cat > "$DEPLOY_DIR/compose.yaml" << COMPOSE_EOF
# ============================================================
# SignalHunter — Docker Compose
# Gerado automaticamente por create_deploy.sh
#
# Comandos úteis (executar dentro de deploy/):
#   Subir:    docker compose up -d
#   Rebuild:  docker compose up --build -d
#   Logs:     docker compose logs -f signalhunter
#   Parar:    docker compose down
# ============================================================
services:

  mariadb:
    image: mariadb:11
    container_name: signalhunter-mariadb
    restart: unless-stopped
    environment:
      MARIADB_ROOT_PASSWORD: "${DB_ROOT_PASSWORD}"
      MARIADB_DATABASE: signalhunter
      MARIADB_USER: signalhunter_user
      MARIADB_PASSWORD: "${DB_PASSWORD}"
    volumes:
      - mariadb_data:/var/lib/mysql
    networks:
      - sh_internal
    healthcheck:
      test: ["CMD", "healthcheck.sh", "--connect", "--innodb_initialized"]
      start_period: 15s
      interval: 10s
      timeout: 5s
      retries: 6

  signalhunter:
    image: signalhunter:local
    build:
      context: ..
      dockerfile: deploy/Dockerfile
    container_name: signalhunter
    restart: unless-stopped
    depends_on:
      mariadb:
        condition: service_healthy
    ports:
      - "${APP_PORT}:${APP_PORT}"
    volumes:
      - ./config/config.toml:/etc/signalhunter/config.toml:ro
      - sh_reports:/opt/signalhunter/reports
    tmpfs:
      - /tmp:size=128m,mode=1777
    networks:
      - sh_internal
    user: "1000:1000"

volumes:
  mariadb_data:
  sh_reports:

networks:
  sh_internal:
    driver: bridge
COMPOSE_EOF
    ok "compose.yaml criado"
}

# ============================================================
# Perguntar porta e verificar se está em uso
# ============================================================
ask_port() {
    while true; do
        read -r -p "$(echo -e "${BOLD}Informe a porta em que a aplicação irá rodar [ex: 8080]:${NC} ")" APP_PORT
        # Validar se é número e está no range válido
        if ! [[ "$APP_PORT" =~ ^[0-9]+$ ]] || [[ "$APP_PORT" -lt 1 || "$APP_PORT" -gt 65535 ]]; then
            warn "Porta inválida. Informe um número entre 1 e 65535."
            continue
        fi
        # Verificar se a porta está em uso
        if ss -tlnH "sport = :${APP_PORT}" 2>/dev/null | grep -q ":${APP_PORT}" || \
           ss -ulnH "sport = :${APP_PORT}" 2>/dev/null | grep -q ":${APP_PORT}"; then
            warn "A porta ${APP_PORT} já está em uso. Escolha outra."
        else
            ok "Porta ${APP_PORT} disponível."
            break
        fi
    done
}

# ============================================================
# MODO: deploy  (primeira instalação)
# ============================================================
do_deploy() {
    echo ""
    echo -e "${BOLD}${CYAN}================================================${NC}"
    echo -e "${BOLD}${CYAN}   SignalHunter — PRIMEIRA INSTALAÇÃO           ${NC}"
    echo -e "${BOLD}${CYAN}================================================${NC}"
    echo ""

    check_deps
    ask_port

    # Verificar se já existe
    if [[ -d "$REPO_DIR" ]]; then
        die "O diretório '$REPO_DIR' já existe.\n" \
            "Para atualizar use: $(basename "$0") update\n" \
            "Para reinstalar do zero, remova manualmente:\n  rm -rf '$REPO_DIR' '$DEPLOY_DIR'"
    fi

    # Clonar repositório
    log "Clonando repositório (branch: $BRANCH)..."
    git clone --branch "$BRANCH" "$REPO_URL" "$REPO_DIR"
    ok "Repositório clonado → $REPO_DIR"

    # Criar estrutura de deploy
    mkdir -p "$DEPLOY_DIR/config"
    log "Criando estrutura em: $DEPLOY_DIR"

    # Gerar e escrever todos os arquivos
    gen_secrets
    write_dockerfile
    write_dockerignore
    write_config
    write_compose
    save_secrets

    # Mostrar estrutura criada
    echo ""
    echo -e "${BOLD}Estrutura criada:${NC}"
    echo "  $(basename "$WORK_DIR")/"
    echo "  ├── .dockerignore"
    echo "  ├── signalhunter/            ← código-fonte (GitHub)"
    echo "  └── deploy/"
    echo "      ├── Dockerfile"
    echo "      ├── compose.yaml"
    echo "      ├── secrets.txt          ← GUARDE COM SEGURANÇA"
    echo "      └── config/"
    echo "          └── config.toml"
    echo ""

    # Mostrar senhas na tela uma vez
    echo -e "${BOLD}${YELLOW}================================================${NC}"
    echo -e "${BOLD}${YELLOW}  SENHAS GERADAS — guarde em local seguro!     ${NC}"
    echo -e "${BOLD}${YELLOW}================================================${NC}"
    echo -e "  DB_PASSWORD      : ${DB_PASSWORD}"
    echo -e "  DB_ROOT_PASSWORD : ${DB_ROOT_PASSWORD}"
    echo -e "  MASTER_KEY       : ${MASTER_KEY}"
    echo -e "  JWT_SECRET       : ${JWT_SECRET}"
    echo -e "${BOLD}${YELLOW}================================================${NC}"
    echo ""

    # Build e start
    log "Iniciando build e deploy (primeira compilação pode levar alguns minutos)..."
    cd "$DEPLOY_DIR"
    docker compose up --build -d

    echo ""
    ok "Containers em execução!"
    echo ""
    echo -e "${BOLD}${YELLOW}⚠  Anote a senha do admin nos logs abaixo!${NC}"
    echo -e "${CYAN}   Pressione Ctrl+C para sair dos logs quando copiar a senha.${NC}"
    echo ""
    sleep 2
    docker compose logs -f signalhunter
}

# ============================================================
# MODO: update  (atualização do GitHub + rebuild)
# ============================================================
do_update() {
    echo ""
    echo -e "${BOLD}${CYAN}================================================${NC}"
    echo -e "${BOLD}${CYAN}   SignalHunter — ATUALIZAÇÃO                   ${NC}"
    echo -e "${BOLD}${CYAN}================================================${NC}"
    echo ""

    check_deps

    [[ -d "$REPO_DIR" ]] || \
        die "Repositório não encontrado em '$REPO_DIR'.\nExecute '$(basename "$0") deploy' primeiro."

    [[ -d "$DEPLOY_DIR" ]] || \
        die "Pasta deploy não encontrada em '$DEPLOY_DIR'.\nExecute '$(basename "$0") deploy' primeiro."

    [[ -f "$DEPLOY_DIR/compose.yaml" ]] || \
        die "compose.yaml não encontrado. Execute '$(basename "$0") deploy' para reinstalar."

    log "Sincronizando signalhunter com branch '$BRANCH'..."
    git -C "$REPO_DIR" fetch origin
    git -C "$REPO_DIR" reset --hard "origin/$BRANCH"
    ok "Código atualizado → commit: $(git -C "$REPO_DIR" log -1 --format='%h %s')"

    log "Reconstruindo imagem e reiniciando containers..."
    cd "$DEPLOY_DIR"
    docker compose up --build -d

    echo ""
    ok "Atualização concluída!"
    echo ""
    log "Status dos containers:"
    docker compose ps
    echo ""
    log "Últimas linhas de log:"
    docker compose logs --tail=20 signalhunter
}

# ============================================================
# MODO: clean  (para containers, remove volumes e arquivos)
# ============================================================
do_clean() {
    echo ""
    echo -e "${BOLD}${RED}================================================${NC}"
    echo -e "${BOLD}${RED}   SignalHunter — LIMPEZA COMPLETA              ${NC}"
    echo -e "${BOLD}${RED}================================================${NC}"
    echo ""
    warn "Esta operação irá:"
    echo "  • Parar e remover os containers"
    echo "  • Remover os volumes Docker (banco de dados será apagado!)"
    echo "  • Remover a pasta deploy/  ($DEPLOY_DIR)"
    echo "  • Remover a pasta signalhunter/  ($REPO_DIR)"
    echo "  • Remover o arquivo .dockerignore"
    echo ""
    read -r -p "$(echo -e "${BOLD}${RED}Confirma a limpeza completa? [s/N]:${NC} ")" CONFIRM
    case "$CONFIRM" in
        [sS])
            ;;
        *)
            echo ""
            warn "Operação cancelada pelo usuário."
            exit 0
            ;;
    esac

    echo ""

    # Parar e remover containers + volumes
    if [[ -f "$DEPLOY_DIR/compose.yaml" ]]; then
        log "Parando containers e removendo volumes..."
        cd "$DEPLOY_DIR"
        docker compose down -v 2>/dev/null || true
        ok "Containers e volumes removidos"
    else
        warn "compose.yaml não encontrado — pulando remoção de containers"
    fi

    # Remover imagem local
    if docker image inspect signalhunter:local &>/dev/null; then
        log "Removendo imagem signalhunter:local..."
        docker image rm signalhunter:local 2>/dev/null || true
        ok "Imagem removida"
    fi

    # Remover pasta deploy/
    if [[ -d "$DEPLOY_DIR" ]]; then
        log "Removendo $DEPLOY_DIR ..."
        rm -rf "$DEPLOY_DIR"
        ok "Pasta deploy/ removida"
    fi

    # Remover pasta signalhunter/
    if [[ -d "$REPO_DIR" ]]; then
        log "Removendo $REPO_DIR ..."
        rm -rf "$REPO_DIR"
        ok "Pasta signalhunter/ removida"
    fi

    # Remover .dockerignore
    if [[ -f "$WORK_DIR/.dockerignore" ]]; then
        log "Removendo .dockerignore ..."
        rm -f "$WORK_DIR/.dockerignore"
        ok ".dockerignore removido"
    fi

    echo ""
    ok "Limpeza concluída! Execute '$(basename "$0") deploy' para reinstalar."
}

# ============================================================
# MAIN
# ============================================================
MODE="${1:-}"

case "$MODE" in
    deploy) do_deploy ;;
    update) do_update ;;
    clean)  do_clean  ;;
    *)
        echo ""
        echo -e "${BOLD}SignalHunter — Script de Deploy${NC}"
        echo ""
        echo "Uso: $(basename "$0") {deploy|update|clean}"
        echo ""
        echo "  deploy  — Primeira instalação completa:"
        echo "            clone do GitHub, criação de arquivos, geração de senhas,"
        echo "            build da imagem Docker e start dos containers."
        echo ""
        echo "  update  — Atualiza o código com o GitHub (branch: $BRANCH)"
        echo "            e reconstrói os containers sem apagar dados."
        echo ""
        echo "  clean   — Para os containers, remove volumes (banco de dados),"
        echo "            imagem Docker e todos os arquivos gerados pelo deploy."
        echo "            Solicita confirmação antes de executar."
        echo ""
        exit 1
        ;;
esac
