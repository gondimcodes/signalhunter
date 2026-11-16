# Changelog — SignalHunter

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

---

## [1.2.1] — 2026-11-16

### ⚡ UI/UX Enhancements & Real-Time PON Active ONU Telemetry
- **Active ONU Count Badge on PON Audit Modal**:
  - Integrated a real-time active ONU capacity badge (`[ X ONUs Ativas ]`) directly into the `#ponAuditModal` header title, immediately adjacent to the PON interface identifier (`S{slot}/P{pon_port}`).
  - Leverages the strict online/active ONU calculation from the Rust analytical engine (`OpticalEvaluator::evaluate_upstream_diagnostics`), displaying accurate active client density without offline/LOS noise.
  - Refined modal subtitle layout by removing redundant text, maintaining focus on OLT identification and average upstream reception power (`avg_olt_rx_dbm`).

---

## [1.2.0] — 2026-11-14

### 🚀 PON Upstream Health, Bidirectional Optical Diagnostics & APD Detection
- **Bidirectional Telemetry Breakdown in ONU Diagnostics Modal**:
  - Expanded `showOnuAuditModal` to display a 6-card analytical grid:
    - `Rx ONU (Downstream)` and `Tx ONU (Upstream)`
    - `Rx OLT (PON Reception)` and `Tx OLT (PON SFP Output)`
    - `Optical Asymmetry (Δ)` and `Transceiver Temperature`
  - Upgraded temporal audit history to 7 columns with native bidirectional telemetry (`Rx ONU`, `Tx ONU`, `Rx OLT`, `Tx OLT`, `ΔdB`, `Status/Cause`).
  - Added `latest_olt_tx_power_dbm` across `OnuRecord` and all underlying SQL queries.
- **Dedicated PON Upstream Audit Panel & Interactive Port Modal**:
  - Added new top-level tab `Auditoria Upstream & PONs` (`#content-upstream`) with real-time KPI metrics:
    - Saturação Upstream ($> -12.0\text{ dBm}$)
    - Atenção Upstream ($-25.0$ to $-27.0\text{ dBm}$)
    - Rx OLT Crítico ($\le -27.0\text{ dBm}$)
    - Assimetria Óptica ($\ge 3.0\text{ dB}$)
    - Portas PON com Suspeita APD (collective optical loss at SFP/DIO)
  - Interactive PON Audit Modal: clicking any PON card opens a dedicated modal displaying all problematic ONUs on that specific interface with quick search, severity badges, and direct links to ONU diagnostics.
  - Saturação Upstream rule refinement: strictly triggered by PON port average reception (`avg_rx > -12.00 dBm`), eliminating false positives from isolated ONUs.
  - Strict exclusion of offline / LOS ONUs from upstream calculations, clearing residual SNMP cache readings (`-3.91 dBm`, `-3.56 dBm`).
- **Executive PDF Report ("Auditoria de Upstream & Saúde das Portas PON")**:
  - Implemented high-density PDF report generation endpoint `/api/reports/pdf?type=upstream`.
  - Section 1: Executive table of suspect PON ports with APD/DIO failure risk.
  - Section 2: Problematic ONUs grouped by OLT and ordered strictly from worst to best Rx OLT (Saturation $> -12\text{ dBm}$ $\rightarrow$ Critical $\le -27\text{ dBm}$ $\rightarrow$ Attention $-25\text{ to }-27\text{ dBm}$ $\rightarrow$ Optical Asymmetry).
  - Clean column formatting avoiding text collisions and supporting multi-page flows.
- **Clean Architecture, Zero Warnings & Dependency Pruning**:
  - Pruned unused dependencies (`axum-extra`, `futures`, `quick-xml`, `tower-http`) from `Cargo.toml`, reducing dependencies from 346 to 328.
  - Added comprehensive unit tests for upstream evaluation, PON saturation thresholds, and LOS exclusion.
  - 100% compliant with `cargo fmt`, `cargo clippy -- -D warnings`, and 12/12 unit tests passing.

---

## [1.1.2] — 2026-11-02

### 🛡️ Security & Supply-Chain Hardening
- **Comprehensive RustSec Advisory Remediation (Zero Vulnerabilities)**:
  - Remediated 10 security advisories and 12 warnings via selective dependency tree upgrade:
    - `zstd` (0.13.3 -> 0.14.3 / RUSTSEC-2026-0391: stream context leak prevention).
    - `zstd-safe` (7.2.4 -> 8.2.0 / RUSTSEC-2026-0398 & RUSTSEC-2026-0403: memory safety & buffer validation).
    - `zstd-sys` (2.0.16 -> 2.1.2 / RUSTSEC-2026-0410).
    - `rustls` (0.23.45 -> 0.23.46 / RUSTSEC-2026-0349, 0347, 0346, 0348).
    - `rustls-webpki` (0.103.14 -> 0.103.16 / RUSTSEC-2026-0382).
    - `webpki-roots` (1.0.9 -> 1.0.11 / RUSTSEC-2026-0381).
    - `tokio` (1.53.1 -> 1.54.1 / RUSTSEC-2026-0386: I/O safety).
    - `chacha20` (0.10.1 -> 0.10.2: replaced yanked release).
  - Maintained 100% green unit test suite (6/6 passing) and strict `cargo clippy` compliance.

---

## [1.1.1] — 2026-10-13

### 🚀 Driver Resiliency & Vendor Standardization
- **TP-Link DeltaStream Official MIB Alignment**:
  - Migrated primary offline drop cause query to official dynamic event table `.1.3.6.1.4.1.11863.6.107.1.5.2.1.42`, keeping automatic fallback to `.100.1.7.2.1.42` for legacy firmware revisions.
  - Implemented dual drop reason decoding (`TpLinkDriver::decode_down_reason`) handling both textual `DisplayString` and numeric `INTEGER` enums (ITU-T G.984 / DeltaStream standards).
  - Added collection of ONT last online timestamp via `.1.3.6.1.4.1.11863.6.107.1.5.2.1.19`.
  - Added modular chassis detection (`DS-P8000-X2` and `DS-P8000-X7`) via `tpControlBoardTable` (`.1.3.6.1.4.1.11863.6.103.1.1`) in connectivity testing and hardware discovery.
  - Normalized `offline_reason` to be populated strictly when ONUs are offline (`!is_online`), maintaining system-wide state consistency.
  - Added integrated unit test suite for TP-Link index parsing, string/integer drop cause decoding, and power telemetry conversion.

---

## [1.1.0] — 2026-09-19

### 🚀 Driver Resiliency & Bugfixes
- **ZTE Dying Gasp vs LOS Detection and MIB Standardization (C300 / C600 Titan)**:
  - Replaced erroneous `PhaseState` OID (`.8.1.4`) with the official enterprise drop cause OIDs:
    - Primary (MIB 1082 - C600 Titan / modern C300): `.1.3.6.1.4.1.3902.1082.500.10.2.3.8.1.7` (`zxAnGponOnuLastOfflineReason`).
    - Fallback (MIB 1012 - legacy C300): `.1.3.6.1.4.1.3902.1012.3.28.2.1.11` (`zxAnGponOntLastDownCause`).
  - Implemented deterministic enum decoder `ZteDriver::decode_zte_offline_reason`:
    - Values `9` (`onuPowerOff`), `12` (`onuReboot`), `13` (`onuShutdown`), `14`, and `15` normalized to `dying_gasp`.
    - Values `2` (`oltLos`), `3` (`onuLos`), `4` (`onuLof`), and `5` (`onuSf`) normalized to `los`.
    - On legacy MIB 1012: values `1` (`powerOff`) and `4` (`reboot`) mapped to `dying_gasp`.
  - Eliminated false positives where ONU synchronization phase states were incorrectly interpreted as power outages or fiber breaks.

---

## [1.0.9] — 2026-09-04

### 🚀 Enhancements & Driver Resiliency
- **Resilient Multi-Firmware TP-Link DeltaStream Driver**:
  - Implemented dual parsing (`parse_power_value`, `parse_temp_value`, `parse_bias_value`) supporting both `DisplayString` and integer (centi-dBm / centi-degree) SNMP responses across legacy and modern TP-Link DeltaStream firmware revisions.
  - Added hybrid serial decoding (`decode_serial`) supporting raw ASCII, byte arrays, and Hex-STRING formats (`54 50 4C 47...` -> `TPLG...`).
  - Added OLT SFP PON port Tx power collection (`.1.3.6.1.4.1.11863.6.96.1.7.1.1.5`), enabling exact fiber attenuation calculation ($\text{Attenuation} = \text{Tx}_{\text{OLT}} - \text{Rx}_{\text{ONU}}$).
  - Implemented automatic status fallback from `omOnlineStatus` (`.11`) to operational status `.41`.
  - Added native detection and database persistence for TP-Link hardware model (`.1.3.6.1.4.1.11863.6.1.1.5.0`) and firmware version (`.1.3.6.1.4.1.11863.6.1.1.6.0`).

---

## [1.0.8] — 2026-09-03

### 🚀 Features & Usability
- **Multi-Token Concatenated Search Engine**: Implemented intelligent multi-token search with logical `AND` intersection across the entire frontend SPA (`src/web/index.html`):
  - Allows operators to perform combined queries such as **OLT and PON port** (e.g., `HUAWEI S1/P2`, `ZTE P3`, `FIBERHOME S1/P1:2`).
  - Seamlessly filters records across OLT name, OLT IP, PON port (`sX/pY`, `X/Y`, `pon Y`, `slot X`), ONU serial number, and customer identifier simultaneously.
  - Integrated across all active application tables: **Dashboard Geral** (`#globalSearch`), **ONUs & Sinais** (`#searchOnusInput`), **Piora de Sinal (ΔdB)** (`#searchDegradationInput`), and **Diagnóstico Óptico** (`#searchDiagInput`).
  - Updated input search placeholders with intuitive concatenated query examples.

---

## [1.0.7] — 2026-08-29

### 🛡️ Security & Input Validation
- **Strict User Input Validation**: Enforced alphanumeric character restrictions (`[a-z0-9._-]`) on usernames and active blocking of HTML tags (`<` / `>`) in full name and user management endpoints.

### 🎨 UI & Layout Improvements
- **Users Table Full-Width & Overflow Handling**: Expanded user management container to 100% width, applied fixed table layout with text truncation (`text-overflow: ellipsis`) on long names/emails, preventing action buttons from being clipped.

---

## [1.0.6] — 2026-08-29

### 🛡️ Security & Hardening
- **Universal DOM-Based & Stored XSS Mitigation (SEC-01)**: Implemented pure HTML entity sanitization helper (`escapeHtml`) across all dynamic SPA tables, modals, tooltips, and toast notifications in `src/web/index.html`.
- **Production Startup Guard for Sample Secrets (SEC-02)**: Added active validation at daemon startup rejecting default sample secrets for `jwt_secret` and `master_encryption_key` when running in `mode = "production"`.
- **Enforced Strict Session Authentication on All REST & Report Endpoints (SEC-03 / SEC-04)**: Standardized mandatory session validation (`extract_authenticated_session`) across `/api/dashboard`, `/api/diagnostics`, `/api/olts`, `/api/onus`, `/api/onus/:id/history`, and `/api/reports/pdf` without bypasses across all environments.

---

## [1.0.5] — 2026-08-28

### 🚀 Features & Scalability
- **Carrier-Grade Mass Scale Architecture**: Added configurable `max_onus_per_olt` setting in `config.toml` (default: 150,000 ONUs per OLT chassis, exceeding double the physical capacity of top global modular chassis like Huawei MA5800-X17 and ZTE C600 Titan).
- **Unrestricted Inventory & Streaming Queries**: Removed arbitrary 5,000/10,000 limits across REST endpoints (`/api/onus`) and PDF reports, unlocking native support for networks with hundreds of thousands to millions of ONUs.

### 🛡️ Security & Hardening
- **Strict Payload Length Constraints (CWE-400 / CWE-770 fix)**: Enforced strict validation and `maxlength` limits on authentication, user management, and CAPTCHA forms across frontend SPA and Axum backend handlers.
- **Code Style Compliance**: Applied full `cargo fmt` formatting standards across all collector modules and handlers.

---

## [1.0.4] — 2026-08-27

### 🚀 Features & UI/UX
- **Native 100% SNMPv2c Driver for TP-Link DeltaStream GPON OLTs**: Full telemetry and optical diagnostics support for **TP-Link** hardware (`DS-P7001-01`, `DS-P7001-04`, `DS-P7001-08`, `DS-P7001-16`, and `DS-P8000 Series`):
  - Strict ASN.1 OID mappings based on official `tplink-olt-onuManagement.mib` under `.1.3.6.1.4.1.11863.6.100.1.7.2.1`.
  - Hierarchical table index resolution `{omSlotId, omPortId, omOnuId}`.
  - Optical power collection: ONU Rx (`omReceivedOpticalPower`), ONU Tx (`omTransmittedOpticalPower`), and Upstream OLT-Rx (`omOltReceivedOpticalPower`).
  - Physical DDM diagnostics: Laser Bias Current (`omBiasCurrent`), Supply Voltage (`omWorkingVoltage`), Transceiver Temperature (`omWorkingTemperature`), Fiber Distance in meters (`omDistance`), and Last Drop Reason (`omOnuLastDownCauses`).
  - Added hardware showcase entry with 3D studio render for TP-Link DeltaStream chassis in the web interface.
- **Universal Viewport-Adaptive Dynamic Pagination**: Implemented automatic, responsive calculation of records per page across all application tables without vertical scrollbars:
  - **Dashboard Geral (`#onuAlertsTableBody`)**: dynamic calculation (`calculateDashboardPageSize`) ensuring alerts fit the viewport height, with expanded OLT column width (24%) and full-card distribution.
  - **ONUs & Sinais (`#onusFullTableBody` & `#onuHistoryTableBody`)**: independent height calculation for both ONU list and signal history panels (`calculateOnusPageSize`, `calculateHistoryPageSize`) with expanded 52% list panel width.
  - **OLTs (Equipamentos) (`#oltsFullTableBody`)**: dynamic pagination (`calculateOltsPageSize`) seamlessly fitting the hardware showcase view.
  - **Piora de Sinal (`#degradationTableBody`)**: adaptive pagination (`calculateDegradationPageSize`) with 100% full-width layout and inline real-time visualization of the last 5 collected optical Rx power readings formatted with 2 decimal places (`{:.2}`).
  - **Usuários / RBAC (`#usersTableBody`)**: dynamic pagination (`calculateUsersPageSize`) and optimized card layout (`max-width: 960px`).
  - **Logs de Auditoria (`#auditLogsTableBody`)**: dynamic pagination (`calculateAuditPageSize`) and full-width layout with expanded 20% column width for IPv6 addresses and detailed event logs.
- **Interactive Navigation Controls**: Standardized pagination bars featuring dynamic totalizers (`Showing X to Y of Z records (N/page)`), page indicators (`Page X / Y`), and adaptive navigation buttons (`← Prev` / `Next →`).
- **Elimination of Bottom Row Clipping & Whitespace Optimization**: Precise calibration of table row heights (24.2px) and header offsets (28px), fitting +2/3 additional records per page while preventing footer overflow.
- **Unified Real-Time Window Resize & Search Listener**: Global resize debouncing dynamically recalculates table capacities and re-renders active tabs without losing search filters.

---

## [1.0.3] — 2026-08-26

### 🔒 Security & RBAC
- **Live Demo Mode**: Added support for `mode = "demo"` with mutation guards against modifications of accounts and settings by visitors, displaying a `Protected (Demo)` badge.
- **Audit IP Anonymization**: In Demo mode, client IP addresses are masked as `"--"` for operators and visitors, while authenticated administrators retain full visibility of real origin IPs.
- **Layered Client IP Resolution**: Enhanced origin IP detection across reverse proxy headers (`X-Forwarded-For`, `X-Real-IP`, `CF-Connecting-IP`, etc.) with direct native TCP/TLS fallback via `ConnectInfo<SocketAddr>`.
- **RBAC Permission Refinement**: Operators have strict permissions to update exclusively their own password (`/api/users/:id`), with zero privileges to modify roles (`role`) or other user records.

### 📄 Reports & PDF
- **PDF Column Alignment & Spacing**: Corrected column alignments and typography spacing in the native PDF engine (`printpdf`), ensuring OLT metadata, hardware models, and last 5 optical power readings render without truncation.

---

## [1.0.2] — 2026-08-25

### ⚡ SNMP Collector & Multi-Vendor
- **OLT Hardware Model & Firmware Auto-Discovery**: Automatic synchronization of hardware models and firmware revisions into the database during periodic background walks.
- **Datacom Factory Serial via MIB `.38`**: Implemented capture of real factory serial numbers (`onuIfSerialNumber` at OID `.1.3.6.1.4.1.3709.3.6.2.1.1.38`) for Datacom DmOS chassis.
- **ZTE Multi-Family Support (Titan C600 & C300)**: Mapped optical diagnostic subtrees and disconnect cause MIBs (`zxAnGponOntLastDownCause`) with exact differentiation between *Dying Gasp* and *LOS*.
- **Huawei Firmware via RFC 2737**: Added software version parsing via `entPhysicalSoftwareRev` with corporate fallback on `hwGponDevSoftwareVersion` for MA5800 and MA5600 OLTs.
- **SNMP Infinite Loop Guard**: Added safety check in `bulk_walk` algorithm to protect against non-conformant OLTs that fail to advance lexicographical OID pointers.

### 🗄️ Database & Performance
- **OLT History Index**: Created composite index `idx_onus_olt_id` to accelerate history lookups and cascade deletions by OLT.
- **Legacy Field Pruning**: Purged unused SSH/Netconf and SNMPv3 (`snmp_v3_*`) database columns, unifying schema to 100% SNMPv2c.
- **Eliminated Synchronous Boot `ALTER TABLE`**: Removed heavy schema modifications during daemon boot, preventing I/O stalls on high-volume tables.

### 🖥️ UI & Reports
- **Executive Reports Dropdown**: Added navigation bar dropdown with native PDF exports for OLT Models/Firmwares, General ONU Inventory, and Optical Degradation ($\Delta\text{dB}$).
- **Modal Z-Index Fix**: Elevated modal overlay `z-index` to `90000+` to reliably overlay fixed navigation headers.

---

## [1.0.1] — 2026-08-25

### ⚡ Dashboard Performance
- **Indexed Correlated Subquery Aggregation**: Optimized `/api/dashboard/metrics` route reducing computation to sub-millisecond execution ($O(1)$ per active ONU) backed by composite index `idx_onu_latest_id (onu_id, id DESC)`.
- **24-Hour Default Collection Interval**: Adjusted default periodic polling interval to 1440 minutes (24h) in `config.toml`, minimizing unnecessary optical transceiver wear and database I/O.

### 🔌 Drivers & Protocols
- **ZTE & Huawei 100% SNMPv2c Transition**: Fully eliminated SSH subprocesses in ZTE and Huawei drivers, retrieving offline causes directly through native SNMP MIBs.

---

## [1.0.0] — 2026-08-22

### 🎉 Initial Release
- **High-Concurrency Pure Rust Engine**: Asynchronous backend powered by `tokio` and `axum` with ultra-low memory footprint (<50MB RAM).
- **GPON/EPON Optical Telemetry**: Collection of ONU Rx/Tx, OLT-Rx, attenuation, voltage, bias current, and temperature with continuous delta ($\Delta\text{dB}$) tracking.
- **Automated Root-Cause Analysis (RCA)**: Predictive diagnostic classification (fiber macrobending, dirty connector, laser degradation, power fluctuation).
- **Dark Glassmorphism SPA**: Responsive Cyberpunk interface without heavy JavaScript frameworks, served directly by the Axum binary.
- **Robust SSDLC Security**: JWT authentication encapsulated in `HttpOnly`/`Secure` cookies, anti-XSS protection, cryptographic SHA-256 CAPTCHA hashing, and granular RBAC (`admin`, `operator`, `viewer`).
- **Native PDF Generation**: Built-in executive report generator powered by `printpdf`.
- **Multi-Vendor SNMP Support**: Production-ready drivers for ZTE, Huawei, FiberHome, Datacom, Nokia, and Parks.
- **Automated CI/CD**: Linux automated build and release pipelines (GitHub Actions and Codeberg Woodpecker).
