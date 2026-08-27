# Changelog — SignalHunter

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

---

## [1.0.4] — 2026-08-27

### 🚀 Features & UI/UX
- **Audit Logs 23-Record Pagination**: Implemented fixed 23-records-per-page pagination in the system audit logs tab (`content-audit`), eliminating the vertical scrollbar and ensuring a compact, static layout.
- **Interactive Navigation Controls**: Added pagination bar featuring a dynamic totalizer (`Showing X to Y of Z records`), page indicator (`Page X / Y`), and interactive navigation buttons (`← Prev (23)` / `Next (23) →`).
- **Real-Time Search Retention**: Instant textual search across audit logs continues filtering against the full dataset loaded from the server, automatically recalculating pages starting from page 1.

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
