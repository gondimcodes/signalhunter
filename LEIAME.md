# SignalHunter
### Sistema Inteligente de Telemetria Óptica, Diagnóstico de Degradação ($\Delta\text{dB}$) e Monitoramento GPON / EPON de Alta Performance

![SignalHunter Logo](logo.png)

> **Desenvolvido para Provedores de Internet (ISPs), Operadoras de Telecomunicações e Equipes de NOC.**  
> Auditoria contínua de parâmetros físicos ópticos (Rx, Tx, OLT-Rx, Atenuação, Temperatura, Distância e Voltagem), histórico granular e diagnósticos preditivos automatizados de causa-raiz.

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

O SignalHunter possui drivers de engenharia especializados em Rust nativo para os principais fabricantes de telecomunicações do mercado. A arquitetura foi concebida para priorizar **SNMPv2c** de alta velocidade em todos os dados disponíveis e recorrer ao **SSH cirúrgico** (com *Inactivity Watchdog*) apenas para informações exclusivas de CLI:

### 📊 Matriz de Protocolos de Coleta por Fabricante

| Fabricante | Modelos Homologados | Apenas SNMPv2c | SNMPv2c + SSH | Papel do SSH (Quando Utilizado) |
| :--- | :--- | :---: | :---: | :--- |
| **Huawei** | SmartAX MA5800, MA5608T, MA5680T | ❌ | ✅ **Sim** | Validação de alarmes de queda (Dying Gasp / LOS) e double-check de telemetria |
| **Datacom** | DmOS DM4610, DM4615, DM4618 | ❌ | ✅ **Sim** | Fallback de distância em ONUs sem bridge L2 e extração de alarmes OMCI |
| **ZTE** | ZXA10 C600, C650, C610 (Titan), C300, C320 | ❌ | ✅ **Sim** | Leitura instantânea de `Phase State` (Dying Gasp vs LOS) e validação cruzada |
| **FiberHome** | AN5516-01, AN5516-04, AN5516-06 | ✅ **Exclusivo** | ❌ *Desabilitado* | **100% via SNMPv2c**: Rx, Tx, SFP Tx Real, OLT-Rx Calculado, Temp, Tensão, Bias, Distância e Nomes |
| **Nokia / Alcatel** | ISAM 7360 FX, 7342, 7330, Lightspan FX | ✅ **Exclusivo** | ❌ *Desabilitado* | **100% SNMPv2c**: DDM completo, Seriais, Alarmes `.88` (Dying Gasp vs LOS), Rx, Tx, OLT-Rx e Nomes |
| **Parks** | Fiberlink 30028, 21000, 21016, 21008, 21004 | ✅ **Exclusivo** | ❌ *Desabilitado* | **100% SNMPv2c**: Rx centi-dBm (`.15`), Temp (`.6.1.10`), Nomes (`.62`), Dying Gasp vs LOS (`.41`/`.5`) |

---

### 📋 Detalhamento dos Parâmetros Coletados

| Fabricante | Parâmetros Coletados |
|---|---|
| **Huawei** | Serial (Hex/ASCII), Rx ONU, Tx ONU, OLT-Rx Upstream, Distância Métrica (m), Causa da Queda (Dying Gasp / LOS), Temperatura, Tensão, Nome do Cliente |
| **Datacom** | Serial, Rx ONU, Distância Decimal (km/m), Nomes de Clientes, Potência Tx SFP PON, Alarmes OMCI de Queda, Uptime |
| **ZTE** | Serial (ASCII/Hex), Rx ONU, Tx ONU, OLT-Rx Upstream, Atenuação ($\text{dB}$), Distância (m), Temp, Tensão, Nome, Phase State |
| **FiberHome** | Serial, Rx ONU, Tx ONU, OLT SFP Tx Real (`.800.3.9.3.4.1.8`), OLT-Rx Calculado, Temperatura, Tensão, Bias Current, Distância (m), Nomes de Clientes, Status Operacional |
| **Nokia / Alcatel** | Serial (Hex/ALCL), Rx ONU, Tx ONU, OLT-Rx Upstream, Atenuação ($\text{dB}$), Distância (m), Temp, Voltagem, Bias, Modelo ONT, Queda Diferenciada (`.88`) |
| **Parks** | Serial (Hex/ASCII), Rx ONU centi-dBm (`.15`), Nomes de Clientes (`.62`), Temperatura do Transceiver (`.6.1.10`), Causa de Queda Dying Gasp vs LOS (`.41`/`.5`), Tx ONU Calibrado e OLT-Rx |

---

### 📡 Mapeamento Técnico de OIDs & Comandos de Validação

#### 1. Huawei (VRP / SmartAX MA5800 & MA5600T Series)
A coleta na Huawei opera em **Rust puro** com pipeline assíncrono balanceado:

* **Tabelas SNMP Enterprise (`HUAWEI-XPON-MIB`):**
  * `Distância Física da Fibra (m)`: `.1.3.6.1.4.1.2011.6.128.1.1.2.46.1.20.<ifIndex>.<onuId>` (Inteiro em metros)
  * `Potência Óptica Rx da ONU (dBm)`: `.1.3.6.1.4.1.2011.6.128.1.1.2.51.1.4.<ifIndex>.<onuId>` (Valor / 100)
  * `Potência Óptica Tx da ONU (dBm)`: `.1.3.6.1.4.1.2011.6.128.1.1.2.51.1.3.<ifIndex>.<onuId>` (Valor / 100)
  * `Potência Óptica Rx OLT Upstream (dBm)`: `.1.3.6.1.4.1.2011.6.128.1.1.2.51.1.6.<ifIndex>.<onuId>` ((Valor - 10000) / 100)
  * `Potência Tx Módulo SFP PON OLT (dBm)`: `.1.3.6.1.4.1.2011.6.128.1.1.2.23.1.2.<ifIndex>` (Valor / 100)
  * `Serial / PON Identifier`: `.1.3.6.1.4.1.2011.6.128.1.1.2.43.1.3.<ifIndex>.<onuId>`
  * `Nome / Descrição do Cliente`: `.1.3.6.1.4.1.2011.6.128.1.1.2.43.1.9.<ifIndex>.<onuId>`
  * `Modelo do Equipamento ONT`: `.1.3.6.1.4.1.2011.6.128.1.1.2.45.1.4.<ifIndex>.<onuId>`
  * `Causa da Última Queda`: `.1.3.6.1.4.1.2011.6.128.1.1.2.47.1.3.<ifIndex>.<onuId>` (`1` = Dying Gasp / Energia, `2`/`3` = LOS / Fibra Rompida)
  * `Temperatura do Transceiver (°C)`: `.1.3.6.1.4.1.2011.6.128.1.1.2.51.1.1.<ifIndex>.<onuId>`
  * `Tensão de Alimentação (V)`: `.1.3.6.1.4.1.2011.6.128.1.1.2.51.1.2.<ifIndex>.<onuId>` (Valor / 100)

* **Comandos SSH de Validação e Diagnóstico (Modo Consulta):**
  ```text
  display ont optical-info <frame>/<slot>/<port> <ont-id>
  display ont info <frame>/<slot>/<port> <ont-id>
  ```

---

#### 2. Datacom (DmOS DM4610 / DM4615 / DM4618)
A Datacom opera prioritariamente via **SNMPv2c**, com **Streaming Line-by-Line e Watchdog de Inatividade** no SSH:

* **Tabelas SNMP Enterprise (`DATACOM-DMOS-GPON-MIB`):**
  * `Potência Óptica Rx da ONU (dBm)`: `.1.3.6.1.4.1.3709.3.6.2.1.1.22.<ifIndex>` (String decimal exata)
  * `Distância Decimal da Fibra (km)`: `.1.3.6.1.4.1.3709.3.6.2.1.1.21.<ifIndex>` (String decimal, ex: "2.35" km = 2350m)
  * `Nome / Identificador do Assinante`: `.1.3.6.1.4.1.3709.3.6.2.1.1.5.<ifIndex>`
  * `Potência Tx Módulos SFP PON OLT (dBm)`: `.1.3.6.1.4.1.3709.3.6.8.2.1.1.3.<portIndex>` (Valor / 100)
  * `Interface L2 / Mapeamento de Porta`: `.1.3.6.1.4.1.3709.3.6.2.1.1.3.<ifIndex>` (Ex: "gpon-1/1/1-onu-0")
  * `Uptime de Conexão da ONU`: `.1.3.6.1.4.1.3709.3.6.2.1.1.26.<ifIndex>` (TimeTicks)

* **Comandos SSH de Validação Estrutural e OMCI (Streaming com Inactivity Watchdog):**
  ```text
  show interface gpon onu
  show interface transceivers gpon
  show interface gpon <slot>/<shelf>/<port> onu <onu-id> | display curly-braces
---

#### 3. ZTE (ZXA10 C300 / C320 / C600 / C610 Titan Series)
* `Potência Óptica Rx da ONU`: `.1.3.6.1.4.1.3902.1082.500.1.2.4.2.1.1.<portIndex>.<onuId>` (raw / 1000 - 100 dBm)
* `Potência Óptica Tx da ONU`: `.1.3.6.1.4.1.3902.1082.500.1.2.4.2.1.2.<portIndex>.<onuId>`
* `Potência Rx OLT Upstream`: `.1.3.6.1.4.1.3902.1082.500.1.2.4.2.1.3.<portIndex>.<onuId>`
* `Distância da Fibra (m)`: `.1.3.6.1.4.1.3902.1082.500.10.2.3.8.1.4.<portIndex>.<onuId>`
* `Status / Phase State (Dying Gasp vs LOS)`: `.1.3.6.1.4.1.3902.1082.500.10.2.3.8.1.11.<portIndex>.<onuId>`

* **Comandos SSH de Validação e Diagnóstico (Modo Consulta):**
  ```text
  show gpon onu detail-info gpon-onu_<port>:<onu-id>
  show pon power ont gpon-onu_<port>:<onu-id>
  ```

---

#### 4. FiberHome (AN5516-01 / AN5516-04 / AN5516-06 Series)
A coleta na FiberHome opera **100% via SNMPv2c**:
* `Potência Tx SFP PON Real OLT (dBm)`: `.1.3.6.1.4.1.5875.800.3.9.3.4.1.8.<slot>.<port>` (raw / 100.0)
* `Potência Óptica Rx da ONU (dBm)`: `.1.3.6.1.4.1.5875.800.3.9.3.3.1.6.<slot>.<port>.<onu>` (raw / 100.0)
* `Potência Óptica Tx da ONU (dBm)`: `.1.3.6.1.4.1.5875.800.3.9.3.3.1.7.<slot>.<port>.<onu>` (raw / 100.0)
* `Temperatura do Transceiver (°C)`: `.1.3.6.1.4.1.5875.800.3.9.3.3.1.8.<slot>.<port>.<onu>` (raw / 10.0)
* `Tensão de Alimentação (V)`: `.1.3.6.1.4.1.5875.800.3.9.3.3.1.9.<slot>.<port>.<onu>` (raw / 100.0)
* `Corrente de Bias (mA)`: `.1.3.6.1.4.1.5875.800.3.9.3.3.1.10.<slot>.<port>.<onu>` (raw / 1000.0)
* `Distância Física da Fibra (m)`: `.1.3.6.1.4.1.5875.800.3.9.3.3.1.11.<slot>.<port>.<onu>`

---

#### 5. Nokia / Alcatel-Lucent (ISAM 7360 FX / Lightspan FX Series)
A coleta na Nokia opera **100% via SNMPv2c** com isolamento total de SSH:
* `Seriais das ONUs`: `.1.3.6.1.4.1.637.61.1.35.10.1.1.5`
* `Status Operacional da ONU`: `.1.3.6.1.4.1.637.61.1.35.10.4.1.8` (`12`/`1` = Online)
* `Diagnóstico de Queda (Dying Gasp vs LOS)`: `.1.3.6.1.4.1.637.61.1.35.10.1.1.88` (`256` = Dying Gasp / Falta de Energia, `2` = LOS / Fibra Rompida, `1` = Online)
* `Potência Óptica Rx da ONU (dBm)`: `.1.3.6.1.4.1.637.61.1.35.10.14.1.2` (raw * 0.002 dBm)
* `Potência Óptica Tx da ONU (dBm)`: `.1.3.6.1.4.1.637.61.1.35.10.14.1.3`
* `Potência Óptica Rx OLT Upstream (dBm)`: `.1.3.6.1.4.1.637.61.1.35.10.18.1.2` (raw / 10.0 dBm)

---

#### 6. Parks (Fiberlink 30028 / 21000 / 21016 Series)
A coleta na Parks opera **100% via SNMPv2c** com isolamento total de SSH:
* `Seriais das ONUs (Hex/ASCII)`: `.1.3.6.1.4.1.6771.10.1.5.1.18.<slot>.<port>.<onu>` (Ex: `PRKS00C418A1`)
* `Nome / Login do Assinante`: `.1.3.6.1.4.1.6771.10.1.5.1.62.<slot>.<port>.<onu>` (Ex: `NelsonAmorim`)
* `Modelo da ONT (Hex/ASCII)`: `.1.3.6.1.4.1.6771.10.1.5.1.23.<slot>.<port>.<onu>` (Ex: `Fiberlink101`)
* `Potência Óptica Rx da ONU (dBm)`: `.1.3.6.1.4.1.6771.10.1.5.1.15.<slot>.<port>.<onu>` (Escala centi-dBm: `2638` $\rightarrow -26.38\text{ dBm}$)
* `Temperatura do Transceiver (°C)`: `.1.3.6.1.4.1.6771.10.1.6.1.10.<slot>.<port>.<onu>.2` (raw / 10.0)
* `Diferenciação de Queda (Dying Gasp vs LOS)`: `.1.3.6.1.4.1.6771.10.1.5.1.41` (`1` = Dying Gasp / Falta de Energia, `0` = LOS / Fibra Rompida) e `.1.3.6.1.4.1.6771.10.1.5.1.5` (`3` = Online, `1` = Dying Gasp, `0` = LOS)

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
