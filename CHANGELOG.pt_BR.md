# Registro de Alterações (Changelog) — SignalHunter

Todas as alterações notáveis deste projeto serão documentadas neste arquivo.

O formato é baseado no [Keep a Changelog](https://keepachangelog.com/pt-BR/1.0.0/),
e este projeto adere ao [Semantic Versioning](https://semver.org/lang/pt-BR/).

---

## [1.0.4] — (Em Desenvolvimento)

### 🚀 Funcionalidades & Interface (UI/UX)
- **Paginação de 23 em 23 nos Logs de Auditoria**: Implementada paginação de exatamente 23 registros por página na aba de auditoria (`content-audit`), eliminando a barra de rolagem lateral e mantendo layout compacto e estático.
- **Controles Interativos de Navegação**: Adicionada barra de paginação com indicador `Pág X / Y`, totalizador dinâmico `Mostrando X a Y de Z registros` e botões de `← Anterior (23)` / `Próxima (23) →`.
- **Preservação de Busca em Tempo Real**: O mecanismo de busca instantânea nos logs de auditoria continua pesquisando sobre toda a base carregada do servidor, recalculando as páginas em tempo real a partir da página 1.

---

## [1.0.3] — 2026-08-26

### 🔒 Segurança & RBAC
- **Modo Demonstração (Live Demo)**: Adicionado suporte a `mode = "demo"` com proteção contra mutações de contas e dados por visitantes, exibindo badge `Protegido (Demo)`.
- **Anonimização de IP em Auditoria**: No modo Demo, endereços IP de origem são mascarados como `"--"` para operadores e visitantes, preservando visualização real apenas para administradores autenticados.
- **Resolução Precisa de IP com Camadas**: Detecção aprimorada de IP de origem via headers de proxies reversos (`X-Forwarded-For`, `X-Real-IP`, `CF-Connecting-IP`, etc.) com fallback nativo direto TCP/TLS via `ConnectInfo<SocketAddr>`.
- **Refinamento de Permissões de Usuários**: Operadores têm permissão estrita para alterar exclusivamente a sua própria senha (`/api/users/:id`), sem acesso à alteração de perfis (`role`) ou dados de terceiros.

### 📄 Relatórios & PDF
- **Alinhamento e Ajustes de Colunas**: Correção de alinhamentos e espaçamentos no gerador de PDF nativo (`printpdf`), garantindo exibição de metadados, modelos e 5 últimas potências ópticas sem truncamento.

---

## [1.0.2] — 2026-08-25

### ⚡ Coletor SNMP & Multi-Vendor
- **Descoberta Automática de Modelo e Firmware**: Sincronização automática de modelo e versão de firmware das OLTs no banco de dados durante a coleta periódica.
- **Leitura do Serial Real Datacom via MIB `.38`**: Implementada captura do número de série oficial (`onuIfSerialNumber` na OID `.1.3.6.1.4.1.3709.3.6.2.1.1.38`) para chassis Datacom DmOS.
- **Suporte Multi-Família ZTE (Titan C600 e C300)**: Mapeamento de OIDs de diagnóstico óptico e motivos de queda (`zxAnGponOntLastDownCause`) com diferenciação precisa entre *Dying Gasp* e *LOS*.
- **Firmware Huawei via RFC 2737**: Leitura de versão de software via `entPhysicalSoftwareRev` com fallback na MIB corporativa `hwGponDevSoftwareVersion` para OLTs MA5800 e MA5600.
- **Proteção contra Loop Infinito em SNMP**: Proteção no algoritmo `bulk_walk` contra OLTs que não avançam ponteiros de OIDs lexicográficas.

### 🗄️ Banco de Dados & Performance
- **Índice de Histórico por OLT**: Criação do índice `idx_onus_olt_id` para otimizar exclusão e busca de histórico temporal.
- **Eliminação de Campos Legados**: Remoção de colunas de SSH/Netconf e SNMPv3 (`snmp_v3_*`) não utilizadas, unificando o schema em 100% SNMPv2c.
- **Remoção de `ALTER TABLE` Síncronos no Boot**: Eliminação de migrações pesadas no arranque do daemon para prevenir bloqueios de I/O em bases com centenas de milhares de registros.

### 🖥️ Interface & Relatórios
- **Menu Dropdown de Relatórios**: Adicionado menu executivo na barra de navegação com exportação em PDF de Modelos/Firmwares de OLTs, Inventário Geral e Piora de Sinal ($\Delta\text{dB}$).
- **Correção de Z-Index nos Modais**: Modais elevados com `z-index: 90000+` para sobrepor adequadamente menus fixos.

---

## [1.0.1] — 2026-08-25

### ⚡ Performance do Dashboard
- **Subquery Correlacionada Indexada no Dashboard**: Otimização da rota `/api/dashboard/metrics` reduzindo tempo de agregação para submilisegundos ($O(1)$ por ONU ativa) com suporte do índice composto `idx_onu_latest_id (onu_id, id DESC)`.
- **Intervalo Padrão de 24 Horas**: Ajuste do ciclo padrão de varredura periódica para 1440 minutos (24h) no `config.toml`, reduzindo I/O desnecessário e preservando controladoras ópticas.

### 🔌 Drivers & Protocolos
- **Transição de ZTE e Huawei para 100% SNMPv2c**: Eliminação total de chamadas SSH nos drivers ZTE e Huawei, migrando a extração de status de desconexão para MIBs nativas.

---

## [1.0.0] — 2026-08-22

### 🎉 Lançamento Inicial
- **Arquitetura Rust de Alta Concorrência**: Backend assíncrono em `tokio` e `axum` com consumo ultrabaixo de memória (<50MB RAM).
- **Telemetria Óptica GPON/EPON**: Coleta de Rx/Tx ONU, OLT-Rx, atenuação, voltagem, corrente de bias e temperatura com cálculo contínuo de delta ($\Delta\text{dB}$).
- **Diagnóstico Automatizado (RCA)**: Classificação preditiva de falhas (curvatura de fibra, conectorização suja, falha de laser/transceiver, instabilidade de alimentação).
- **Frontend SPA Glassmorphism**: Interface Dark/Cyberpunk sem frameworks pesados, servida diretamente pelo binário Axum.
- **Segurança Robusta (SSDLC)**: Autenticação JWT encapsulada em cookies `HttpOnly` com flag `Secure` condicional, proteção anti-XSS, CAPTCHA criptográfico SHA-256 e controle RBAC (`admin`, `operator`, `viewer`).
- **Exportação Nativa em PDF**: Relatórios executivos de rede e laudos de degradação com `printpdf`.
- **Suporte Multi-Vendor Inicial**: Drivers SNMP para ZTE, Huawei, FiberHome, Datacom, Nokia e Parks.
- **CI/CD Automatizado**: Pipelines de compilação e release para Linux (GitHub Actions e Codeberg Woodpecker).
