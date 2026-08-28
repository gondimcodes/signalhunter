# SignalHunter
### Sistema Inteligente de Telemetria Óptica, Diagnóstico de Degradação ($\Delta\text{dB}$) e Monitoramento GPON / EPON de Alta Performance

> **Desenvolvido para Provedores de Internet (ISPs), Operadoras de Telecomunicações e Equipes de NOC.**  
> Auditoria contínua de parâmetros físicos ópticos (Rx, Tx, OLT-Rx, Atenuação, Temperatura, Distância e Voltagem), histórico granular e diagnósticos preditivos automatizados de causa-raiz.

---

## 🚀 Demonstração Online (Live Demo)

Experimente o SignalHunter em funcionamento agora mesmo através do nosso ambiente oficial de demonstração com dataset simulado multi-vendor:

| 🌐 Link de Acesso | 👤 Usuário | 🔑 Senha |
| :--- | :--- | :--- |
| [**https://signalhunter.ispfocus.net.br:8443**](https://signalhunter.ispfocus.net.br:8443) | `demo` | `demo123` |

---

## 🌐 Visão Geral

O **SignalHunter** é uma plataforma de classe corporativa desenvolvida integralmente em **Rust** assíncrono sobre o runtime `tokio`. Ele foi projetado para resolver o maior gargalo operacional dos provedores de internet: **a degradação silenciosa da rede passiva de fibra óptica (ODN)**.

Em vez de depender de testes manuais reativos após a abertura de chamados pelos clientes, o SignalHunter conecta-se diretamente às OLTs (*Optical Line Terminals*) através de um motor SNMP proprietário de alta resiliência, coletando a telemetria óptica de milhares de ONUs em segundos e calculando automaticamente variações de atenuação ($\Delta\text{dB}$) ao longo do tempo.

---

## ⚡ Vantagens Competitivas

| Vantagem | O que o SignalHunter oferece |
|---|---|
| **🚀 Performance & Concorrência Extrema** | Escrito em Rust puro com compilação nativa. Zero garbage collector, footprint de memória ultra-baixo (< 50 MB de RAM) e capacidade de auditar 50.000+ ONUs simultâneas sem congelar controladoras de OLT. |
| **🛡️ Parser ASN.1 BER SNMP Resiliente** | Motor SNMP proprietário com navegação hierárquica a partir da raiz `SEQUENCE (0x30)`, validação estrita de `Request-ID` (anti-cross contamination) e fallback automático `GetBulk` ➔ `GetNext` ao cruzar limites de placas PON (*card boundary crossing*). |
| **🔍 Detecção Precoce de Degradação ($\Delta\text{dB}$)** | Identifica rompimentos parciais, macrocurvaturas de fibra, atenuações em fusões e conectores sujos antes que a ONU entre em estado crítico de *Loss of Signal* (LOS). |
| **🧠 Diagnóstico Preditivo Inteligente** | Motor analítico que correlaciona queda de sinal coletivo por porta PON, identificando instantaneamente se o problema é generalizado no cabo de distribuição (*feeder*) ou isolado no cliente (*drop*). |
| **📄 Relatórios Executivos em PDF Nativo** | Geração instantânea de relatórios técnicos em PDF com histórico detalhado das últimas 5 coletas por ONU, diagramas de saúde da rede e listagem completa de alertas para equipes de campo. |
| **🔒 Segurança SSDLC & DevSecOps** | Autenticação JWT encapsulada estritamente em cookies `HttpOnly`/`Secure` com proteção anti-XSS, hash criptográfico de CAPTCHA com `SHA-256`, criptografia AES-256-GCM para senhas de OLTs e trilha completa de auditoria RBAC. |
| **💎 Interface SPA Dark/Cyberpunk Neon** | Frontend moderno em HTML5/CSS Glassmorphism sem dependências pesadas (Node/React/Webpack), servido diretamente pelo binário Axum em porta única TLS. |

---

## ✨ Principais Funcionalidades

- **Dashboard Geral em Tempo Real**: Métricas consolidadas de saúde da rede óptica, histograma dinâmico de distribuição de potência (dBm), cartões de qualidade (Saturado, Excelente, Bom, Atenção, Crítico, Offline e Degradação) e tabela interativa de alertas com paginação ultra-rápida de 1.000 em 1.000 registros.
- **Auditoria Individual e Histórico Temporal Granular**: Painel modal e visão lado a lado permitindo auditar linha do tempo de uma ONU com evolução de Rx ONU, Tx ONU, OLT-Rx, Atenuação ($\text{dB}$), Temperatura ($\text{°C}$) e $\Delta\text{Rx}$.
- **Módulo de Piora de Sinal ($\Delta\text{dB}$)**: Filtro especializado para isolar equipamentos que sofreram perda de potência óptica entre coletas consecutivas.
- **Painel de Diagnóstico Óptico e IA de Rede**: Análise probabilística de anomalias ópticas por fabricante e porta PON.
- **Gestão de Equipamentos (OLTs)**: Cadastro centralizado com detecção automática de chassi e firmware, controle de concorrência por equipamento, portas de delay configuráveis e isolamento administrativo de status de operação (*Ativada* vs *Desativada*).
- **Controle de Acesso RBAC & Logs de Auditoria**: Gestão de usuários (`admin`, `operator`, `viewer`) e rastreabilidade total de ações com registro de IP, data/hora e ação executada.

---

## 📸 Demonstração e Telas do Sistema

### 🎬 Demonstração em Loop (Slideshow Interativo de Telas)
![SignalHunter Demonstração](demo.gif)

---
### 1. Dashboard Geral de Monitoramento
Visão executiva com distribuição óptica, indicadores de qualidade global e alertas em tempo real.
![Dashboard Geral](dashboard.png)

---

### 2. Showcase e Auditoria de Hardware Multi-Vendor OLT
Painel interativo com detecção de chassi, firmware, status operacional e especificações por fabricante.

| Nokia / Alcatel-Lucent | Huawei SmartAX GPON |
| :---: | :---: |
| ![Nokia](nokia.png) | ![Huawei](huawei.png) |

| Datacom DmOS | ZTE Titan Series |
| :---: | :---: |
| ![Datacom](datacom.png) | ![ZTE](zte.png) |

| FiberHome AN5516 | Parks Fiberlink |
| :---: | :---: |
| ![FiberHome](fiberhome.png) | ![Parks](parks.png) |

---

### 3. Monitoramento de ONUs, Sinais e Linha do Tempo
Listagem paginada de alta densidade integrada à auditoria detalhada de potência óptica (Rx/Tx, OLT-Rx, atenuação e $\Delta\text{dB}$).
![ONUs e Sinais](sinais.png)

---

### 4. Diagnóstico Óptico Inteligente e RCA
Diagnóstico preditivo automatizado de anomalias ópticas por fabricante e porta PON.
![Diagnóstico Óptico](diagnostico.png)

---

### 5. Trilha e Logs de Auditoria de Segurança
Histórico completo e imutável de todas as ações e eventos executados no sistema.
![Logs de Auditoria](auditoria.png)

---

## 🏭 Suporte Multi-Vendor Homologado

O SignalHunter possui drivers de engenharia especializados em Rust nativo para os principais fabricantes de telecomunicações do mercado. A arquitetura opera **100% via SNMPv2c de alta velocidade** em todos os fabricantes, entregando alto rendimento, coleta não-bloqueante e zero dependência de sessões CLI interativas:

### 📊 Matriz de Protocolos de Coleta por Fabricante

| Fabricante | Modelos Homologados | Compatibilidade de Firmware | Protocolo | Parâmetros Coletados |
| :--- | :--- | :---: | :---: | :--- |
| **Huawei** | SmartAX MA5800, MA5608T, MA5680T | Todas as versões VRP | ✅ **SNMPv2c (100%)** | Seriais, Rx, Tx, SFP Tx PON, OLT-Rx, Distância Métrica (m), Alarmes de Queda (Dying Gasp / LOS), Temp, Tensão e Nomes |
| **Datacom** | DmOS DM4610, DM4615, DM4618 | **DmOS $\ge$ 12.6** | ✅ **SNMPv2c (100%)** | Serial Real (`.38`), Rx (`.22`), Tx (`.21`), Motivo da Última Queda (`.31`), Status Primário (`.37`), SFP PON Tx e Nomes (`.5`) |
| **ZTE** | ZXA10 C600, C650, C610 (Titan), C300, C320 | Todas as versões | ✅ **SNMPv2c (100%)** | Seriais, Rx, Tx, OLT-Rx Calculado, Distância (m), Temp, Tensão, Nomes e Causa de Queda (`.1012.3.28.2.1.4`) |
| **FiberHome** | AN5516-01, AN5516-04, AN5516-06 | Todas as versões | ✅ **SNMPv2c (100%)** | Rx, Tx, SFP Tx Real, OLT-Rx Calculado, Temp, Tensão, Bias, Distância e Nomes |
| **Nokia / Alcatel** | ISAM 7360 FX, 7342, 7330, Lightspan FX | Todas as versões | ✅ **SNMPv2c (100%)** | DDM completo, Seriais, Alarmes `.88` (Dying Gasp vs LOS), Rx, Tx, OLT-Rx e Nomes |
| **Parks** | Fiberlink 30028, 21000, 21016, 21008, 21004 | Todas as versões | ✅ **SNMPv2c (100%)** | Rx centi-dBm (`.15`), Temp (`.6.1.10`), Nomes (`.62`), Dying Gasp vs LOS (`.41`/`.5`) |
| **TP-Link** | DeltaStream DS-P7001 (01/04/08/16), DS-P8000 Series | **Firmware $\ge$ 1.2.0** | ✅ **SNMPv2c (100%)** | Serials (`.6`), Rx (`.26`), Tx (`.27`), OLT-Rx (`.28`), Distância (m) (`.18`), Temp (`.31`), Tensão (`.30`), Bias (`.29`), Nomes (`.5`), Causa de Queda (`.42`) |

---

### 📋 Detalhamento dos Parâmetros Coletados

| Fabricante | Parâmetros Coletados |
|---|---|
| **Huawei** | Número de Série (`.43.1.3`), Nome do Cliente (`.43.1.9`), Modelo ONT (`.45.1.4`), Rx ONU (`.51.1.4`), Tx ONU (`.51.1.3`), OLT-Rx Upstream (`.51.1.6`), Tx SFP PON (`.23.1.2`), Temperatura (`.51.1.1`), Tensão (`.51.1.2`), Causa da Queda (`.47.1.3`), Distância (`.46.1.20`) |
| **Datacom** | Número de Série Real (`.38`), Nome do Cliente (`.5`), Rx ONU (`.22`), Tx ONU (`.21`), Motivo da Queda (`.31`), Status Primário (`.37`), Potência Tx SFP PON (`.3709.3.6.8.2.1.1.3`), Mapeamento de Interface L2 (`.3`) |
| **ZTE** | Número de Série (`.500.20.2.1.2.1.3` / `.300.20.2.1.2.1.3` / `.50.11.2.1.1`), Nome do Cliente (`.500.10.2.3.9.1.2`), Rx ONU (`.500.20.2.2.2.1.10` / `.500.1.2.4.2.1.2`), Tx ONU (`.500.20.2.2.2.1.11` / `.500.1.2.4.2.1.1`), OLT-Rx Upstream (`.500.20.2.2.2.1.12` / `.500.1.2.4.2.1.3`), Distância (`.500.10.2.3.8.1.4`), Temperatura (`.500.20.2.2.2.1.13`), Causa de Queda (`.500.10.2.3.8.1.11` / `.1012.3.28.2.1.4`) |
| **FiberHome** | Número de Série (`.800.3.9.3.3.1.2`), Nome do Cliente (`.800.3.9.3.3.1.4`), Rx ONU (`.800.3.9.3.3.1.6`), Tx ONU (`.800.3.9.3.3.1.7`), Temperatura (`.800.3.9.3.3.1.8`), Tensão (`.800.3.9.3.3.1.9`), Bias Current (`.800.3.9.3.3.1.10`), Distância (`.800.3.9.3.3.1.11`), OLT SFP Tx Real (`.800.3.9.3.4.1.8`) |
| **Nokia / Alcatel** | Número de Série (`.35.10.1.1.5`), Nome do Cliente (`.35.10.1.1.12`), Status Operacional (`.35.10.4.1.8`), Diagnóstico de Queda (`.35.10.1.1.88`), Rx ONU (`.35.10.14.1.2`), Tx ONU (`.35.10.14.1.3`), OLT-Rx Upstream (`.35.10.18.1.2`) |
| **Parks** | Número de Série (`.5.1.18`), Nome / Login (`.5.1.62`), Modelo ONT (`.5.1.23`), Rx ONU (`.5.1.15`), Temperatura do Transceiver (`.6.1.10`), Diagnóstico de Queda (`.5.1.41` / `.5.1.5`) |
| **TP-Link** | Número de Série (`.11863.6.100.1.7.2.1.6`), Nome do Cliente (`.5`), Status Online (`.11`), Vendor ID (`.15`), Modelo do Equipamento (`.16`), Distância (`.18`), Rx ONU (`.26`), Tx ONU (`.27`), OLT-Rx Upstream (`.28`), Corrente de Bias (`.29`), Tensão (`.30`), Temperatura (`.31`), Causa de Queda (`.42`) |

---

### 📡 Mapeamento Técnico de OIDs

#### 1. Huawei (VRP / SmartAX MA5800 & MA5600T Series)
A coleta na Huawei opera **100% via SNMPv2c**:
* `Número de Série da ONU`: `.1.3.6.1.4.1.2011.6.128.1.1.2.43.1.3.<ifIndex>.<onuId>`
* `Nome / Descrição do Cliente`: `.1.3.6.1.4.1.2011.6.128.1.1.2.43.1.9.<ifIndex>.<onuId>`
* `Modelo do Equipamento ONT`: `.1.3.6.1.4.1.2011.6.128.1.1.2.45.1.4.<ifIndex>.<onuId>`
* `Potência Óptica Rx da ONU (dBm)`: `.1.3.6.1.4.1.2011.6.128.1.1.2.51.1.4.<ifIndex>.<onuId>` (Valor / 100)
* `Potência Óptica Tx da ONU (dBm)`: `.1.3.6.1.4.1.2011.6.128.1.1.2.51.1.3.<ifIndex>.<onuId>` (Valor / 100)
* `Potência Óptica Rx OLT Upstream (dBm)`: `.1.3.6.1.4.1.2011.6.128.1.1.2.51.1.6.<ifIndex>.<onuId>` ((Valor - 10000) / 100)
* `Potência Tx Módulo SFP PON OLT (dBm)`: `.1.3.6.1.4.1.2011.6.128.1.1.2.23.1.2.<ifIndex>` (Valor / 100)
* `Temperatura do Transceiver (°C)`: `.1.3.6.1.4.1.2011.6.128.1.1.2.51.1.1.<ifIndex>.<onuId>`
* `Tensão de Alimentação (V)`: `.1.3.6.1.4.1.2011.6.128.1.1.2.51.1.2.<ifIndex>.<onuId>` (Valor / 100)
* `Causa da Última Queda`: `.1.3.6.1.4.1.2011.6.128.1.1.2.47.1.3.<ifIndex>.<onuId>` (`1` = Dying Gasp / Energia, `2`/`3` = LOS / Fibra Rompida)
* `Distância Física da Fibra (m)`: `.1.3.6.1.4.1.2011.6.128.1.1.2.46.1.20.<ifIndex>.<onuId>` (Inteiro em metros)

---

#### 2. Datacom (DmOS DM4610 / DM4615 / DM4618)
A coleta na Datacom opera **100% via SNMPv2c** (requer firmware **DmOS $\ge$ 12.6**):
* `Número de Série da ONU`: `.1.3.6.1.4.1.3709.3.6.2.1.1.38.<ifIndex>` (onuIfSerialNumber)
* `Nome / Descrição do Cliente`: `.1.3.6.1.4.1.3709.3.6.2.1.1.5.<ifIndex>` (onuIfName)
* `Potência Óptica Rx da ONU (dBm)`: `.1.3.6.1.4.1.3709.3.6.2.1.1.22.<ifIndex>` (onuIfOnuPowerRx)
* `Potência Óptica Tx da ONU (dBm)`: `.1.3.6.1.4.1.3709.3.6.2.1.1.21.<ifIndex>` (onuIfOnuPowerTx)
* `Motivo da Última Queda`: `.1.3.6.1.4.1.3709.3.6.2.1.1.31.<ifIndex>` (onuIfLastDownReason)
* `Status Operacional Primário`: `.1.3.6.1.4.1.3709.3.6.2.1.1.37.<ifIndex>` (onuIfPrimaryStatus)
* `Potência Tx Módulos SFP PON OLT (dBm)`: `.1.3.6.1.4.1.3709.3.6.8.2.1.1.3.<portIndex>` (laneTxPower - Valor / 100)
* `Interface L2 / Mapeamento de Porta`: `.1.3.6.1.4.1.3709.3.6.2.1.1.3.<ifIndex>` (onuifDescr, Ex: "gpon-1/1/1-onu-0")

> **Agradecimento Especial**: Um reconhecimento especial e agradecimento a **Tatiane Figueiredo** (`tatiane.figueiredo@gmail.com`) por ajudar o projeto fornecendo a referência oficial de MIBs e auxiliando no mapeamento correto dos OIDs para equipamentos Datacom.

---

#### 3. ZTE (ZXA10 C300 / C320 / C600 / C610 Titan Series)
A coleta na ZTE opera **100% via SNMPv2c**:
* `Número de Série da ONU`: `.1.3.6.1.4.1.3902.1082.500.20.2.1.2.1.3` (Titan C600) / `.1.3.6.1.4.1.3902.1082.300.20.2.1.2.1.3` (C300) / `.1.3.6.1.4.1.3902.1012.3.50.11.2.1.1` (C300 Legacy)
* `Nome / Identificador do Assinante`: `.1.3.6.1.4.1.3902.1082.500.10.2.3.9.1.2.<ifIndex>.<onuId>`
* `Potência Óptica Rx da ONU (dBm)`: `.1.3.6.1.4.1.3902.1082.500.20.2.2.2.1.10` (Titan) / `.1.3.6.1.4.1.3902.1082.500.1.2.4.2.1.2` (C300)
* `Potência Óptica Tx da ONU (dBm)`: `.1.3.6.1.4.1.3902.1082.500.20.2.2.2.1.11` (Titan) / `.1.3.6.1.4.1.3902.1082.500.1.2.4.2.1.1` (C300)
* `Potência Rx OLT Upstream (dBm)`: `.1.3.6.1.4.1.3902.1082.500.20.2.2.2.1.12` (Titan) / `.1.3.6.1.4.1.3902.1082.500.1.2.4.2.1.3` (C300)
* `Temperatura do Transceiver (°C)`: `.1.3.6.1.4.1.3902.1082.500.20.2.2.2.1.13` (Titan) / `.1.3.6.1.4.1.3902.1082.500.1.2.4.2.1.5` (C300)
* `Distância Física da Fibra (m)`: `.1.3.6.1.4.1.3902.1082.500.10.2.3.8.1.4.<ifIndex>.<onuId>`
* `Causa da Última Queda (Dying Gasp vs LOS)`: `.1.3.6.1.4.1.3902.1082.500.10.2.3.8.1.11` / `.1.3.6.1.4.1.3902.1012.3.28.2.1.4` (`1` = Dying Gasp / Energia, `2` = LOS / Fibra Rompida)

---

#### 4. FiberHome (AN5516-01 / AN5516-04 / AN5516-06 Series)
A coleta na FiberHome opera **100% via SNMPv2c**:
* `Número de Série da ONU`: `.1.3.6.1.4.1.5875.800.3.9.3.3.1.2.<slot>.<port>.<onu>`
* `Nome / Identificador do Assinante`: `.1.3.6.1.4.1.5875.800.3.9.3.3.1.4.<slot>.<port>.<onu>`
* `Potência Tx SFP PON Real OLT (dBm)`: `.1.3.6.1.4.1.5875.800.3.9.3.4.1.8.<slot>.<port>` (Raw / 100.0)
* `Potência Óptica Rx da ONU (dBm)`: `.1.3.6.1.4.1.5875.800.3.9.3.3.1.6.<slot>.<port>.<onu>` (Raw / 100.0)
* `Potência Óptica Tx da ONU (dBm)`: `.1.3.6.1.4.1.5875.800.3.9.3.3.1.7.<slot>.<port>.<onu>` (Raw / 100.0)
* `Temperatura do Transceiver (°C)`: `.1.3.6.1.4.1.5875.800.3.9.3.3.1.8.<slot>.<port>.<onu>` (Raw / 10.0)
* `Tensão de Alimentação (V)`: `.1.3.6.1.4.1.5875.800.3.9.3.3.1.9.<slot>.<port>.<onu>` (Raw / 100.0)
* `Corrente de Bias (mA)`: `.1.3.6.1.4.1.5875.800.3.9.3.3.1.10.<slot>.<port>.<onu>` (Raw / 1000.0)
* `Distância Física da Fibra (m)`: `.1.3.6.1.4.1.5875.800.3.9.3.3.1.11.<slot>.<port>.<onu>`

---

#### 5. Nokia / Alcatel-Lucent (ISAM 7360 FX / Lightspan FX Series)
A coleta na Nokia opera **100% via SNMPv2c**:
* `Número de Série da ONU`: `.1.3.6.1.4.1.637.61.1.35.10.1.1.5`
* `Nome / Descrição do Cliente`: `.1.3.6.1.4.1.637.61.1.35.10.1.1.12`
* `Status Operacional da ONU`: `.1.3.6.1.4.1.637.61.1.35.10.4.1.8` (`12`/`1` = Online)
* `Diagnóstico de Queda (Dying Gasp vs LOS)`: `.1.3.6.1.4.1.637.61.1.35.10.1.1.88` (`256` = Dying Gasp / Falta de Energia, `2` = LOS / Fibra Rompida)
* `Potência Óptica Rx da ONU (dBm)`: `.1.3.6.1.4.1.637.61.1.35.10.14.1.2` (Raw * 0.002 dBm)
* `Potência Óptica Tx da ONU (dBm)`: `.1.3.6.1.4.1.637.61.1.35.10.14.1.3`
* `Potência Óptica Rx OLT Upstream (dBm)`: `.1.3.6.1.4.1.637.61.1.35.10.18.1.2` (Raw / 10.0 dBm)

---

#### 6. Parks (Fiberlink 30028 / 21000 / 21016 Series)
A coleta na Parks opera **100% via SNMPv2c**:
* `Número de Série da ONU (Hex/ASCII)`: `.1.3.6.1.4.1.6771.10.1.5.1.18.<slot>.<port>.<onu>` (Ex: `PRKS00C418A1`)
* `Nome / Login do Assinante`: `.1.3.6.1.4.1.6771.10.1.5.1.62.<slot>.<port>.<onu>`
* `Modelo da ONT (Hex/ASCII)`: `.1.3.6.1.4.1.6771.10.1.5.1.23.<slot>.<port>.<onu>`
* `Potência Óptica Rx da ONU (dBm)`: `.1.3.6.1.4.1.6771.10.1.5.1.15.<slot>.<port>.<onu>` (Escala centi-dBm: `2638` $\rightarrow -26.38\text{ dBm}$)
* `Temperatura do Transceiver (°C)`: `.1.3.6.1.4.1.6771.10.1.6.1.10.<slot>.<port>.<onu>.2` (Raw / 10.0)
* `Diagnóstico de Causa de Queda`: `.1.3.6.1.4.1.6771.10.1.5.1.41` (`1` = Dying Gasp / Falta de Energia, `0` = LOS / Fibra Rompida) e `.1.3.6.1.4.1.6771.10.1.5.1.5` (`3` = Online, `1` = Dying Gasp, `0` = LOS)

---

#### 7. TP-Link (DeltaStream DS-P7001-04 / DS-P7001-08 / DS-P7001-16 / DS-P8000 Series)
A coleta na TP-Link DeltaStream opera **100% via SNMPv2c** (indexada por `{slot, port, onu_id}`):
* `Número de Série da ONU`: `.1.3.6.1.4.1.11863.6.100.1.7.2.1.6.<slot>.<port>.<onu>` (`omSerialNumber`)
* `Nome / Descrição do Cliente`: `.1.3.6.1.4.1.11863.6.100.1.7.2.1.5.<slot>.<port>.<onu>` (`omOnuDescription`)
* `Status de Conexão`: `.1.3.6.1.4.1.11863.6.100.1.7.2.1.11.<slot>.<port>.<onu>` (`1` = Online, `0` = Offline)
* `Fabricante / Vendor ID`: `.1.3.6.1.4.1.11863.6.100.1.7.2.1.15.<slot>.<port>.<onu>` (`omVendorId`, ex: `TPLG`, `ZTEG`, `HWTC`)
* `Modelo do Equipamento`: `.1.3.6.1.4.1.11863.6.100.1.7.2.1.16.<slot>.<port>.<onu>` (`omEquipmentId`)
* `Distância Física da Fibra (m)`: `.1.3.6.1.4.1.11863.6.100.1.7.2.1.18.<slot>.<port>.<onu>` (`omDistance`)
* `Potência Óptica Rx da ONU (dBm)`: `.1.3.6.1.4.1.11863.6.100.1.7.2.1.26.<slot>.<port>.<onu>` (`omReceivedOpticalPower`)
* `Potência Óptica Tx da ONU (dBm)`: `.1.3.6.1.4.1.11863.6.100.1.7.2.1.27.<slot>.<port>.<onu>` (`omTransmittedOpticalPower`)
* `Potência Óptica Rx OLT Upstream (dBm)`: `.1.3.6.1.4.1.11863.6.100.1.7.2.1.28.<slot>.<port>.<onu>` (`omOltReceivedOpticalPower`)
* `Corrente de Bias do Laser (mA)`: `.1.3.6.1.4.1.11863.6.100.1.7.2.1.29.<slot>.<port>.<onu>` (`omBiasCurrent`)
* `Tensão de Alimentação (V)`: `.1.3.6.1.4.1.11863.6.100.1.7.2.1.30.<slot>.<port>.<onu>` (`omWorkingVoltage` - Valor bruto em mV / 1000.0)
* `Temperatura do Transceiver (°C)`: `.1.3.6.1.4.1.11863.6.100.1.7.2.1.31.<slot>.<port>.<onu>` (`omWorkingTemperature`)
* `Causa da Última Desconexão`: `.1.3.6.1.4.1.11863.6.100.1.7.2.1.42.<slot>.<port>.<onu>` (`omOnuLastDownCauses`, ex: `LOS`, `Dying Gasp`)

---

## 🎯 Tabela de Classificação de Sinais Ópticos (Downstream Rx)

| Classificação | Faixa de Potência | Cor no Painel | Ação Recomendada |
|---|---|---|---|
| **💎 Excelente** | $-14.00\text{ dBm}$ a $-18.00\text{ dBm}$ | Verde Esmeralda | Operação ótima sem atenuações |
| **✨ Bom / Normal** | $-18.01\text{ dBm}$ a $-23.00\text{ dBm}$ | Ciano / Azul | Operação padrão de rede GPON |
| **⚠️ Atenção** | $-23.01\text{ dBm}$ a $-27.00\text{ dBm}$ | Âmbar / Laranja | Preventivo: verificar fusões e conectores |
| **🚨 Crítico** | $< -27.00\text{ dBm}$ | Vermelho | Manutenção corretiva de campo imediata |
| **⚡ Saturado** | $> -14.00\text{ dBm}$ | Azul Neônio | Inserir atenuador óptico (risco ao fotodiodo) |
| **🔌 Offline (LOS)** | Sem sinal / Desconectado | Cinza Escuro | Rompimento de fibra ou ONU desligada |

---

## 🚀 Instalação e Implantação

Para o procedimento passo a passo de instalação do **Rust**, compilação do código-fonte em modo *release*, configuração do banco de dados **MariaDB/MySQL**, geração de certificados **HTTPS/TLS** e criação do serviço gerenciado no **Systemd** em servidores Linux (Debian 13), consulte o guia completo de instalação:

📖 **[Acesse o Guia de Instalação Completo (INSTALACAO.md)](INSTALACAO.md)**  
*(For the English version, see [INSTALL.md](INSTALL.md))*

---

## ⚖️ Política de Marcas, Logotipos e Isenção de Responsabilidade

Embora o código-fonte deste projeto seja distribuído como Software Livre sob a licença GNU GPLv3, a identidade visual, as logos e o nome empresarial **ISPFocus Serviços e Tecnologia Ltda** são propriedades privadas.

- **Uso Autorizado:** As logos e a identidade visual podem ser utilizadas exclusivamente quando associadas a este software em sua forma original e legítima.
- **Restrições:** É expressamente proibido o uso das logos, da identidade visual ou do nome da empresa em produtos derivados, forks comerciais ou serviços sem autorização prévia por escrito. Para detalhes, consulte [MARCAS.md](MARCAS.md).
- **Isenção de Responsabilidade:** O autor e a **ISPFocus Serviços e Tecnologia Ltda** não se responsabilizam por quaisquer danos diretos, indiretos ou consequenciais decorrentes do uso ou operação deste software. O software é fornecido "no estado em que se encontra" ("AS IS").

---

## 📄 Licença e Suporte

Desenvolvido por **[ISPFocus Serviços e Tecnologia Ltda](https://ispfocus.net.br)**.  
Para suporte comercial, homologação de novos modelos de OLT ou consultoria técnica de redes ópticas, acesse [ispfocus.net.br](https://ispfocus.net.br).
