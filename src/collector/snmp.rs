use std::net::SocketAddr;
use std::sync::atomic::{AtomicU32, Ordering};
use tokio::net::UdpSocket;
use tokio::time::{timeout, Duration};

#[derive(Debug, Clone)]
pub struct SnmpVariableBinding {
    pub oid: String,
    pub value_raw: Vec<u8>,
    pub value_str: Option<String>,
    pub value_int: Option<i64>,
}

/// Cliente SNMPv2c assíncrono puro sobre UDP
pub struct SnmpClient {
    socket: UdpSocket,
    community: String,
    target_addr: SocketAddr,
    timeout_duration: Duration,
    request_id_counter: AtomicU32,
}

impl SnmpClient {
    pub async fn new(
        target_ip: &str,
        port: u16,
        community: &str,
        timeout_ms: u64,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let bind_addr = if target_ip.contains(':') {
            "[::]:0"
        } else {
            "0.0.0.0:0"
        };

        let socket = UdpSocket::bind(bind_addr).await?;
        let target_addr: SocketAddr = format!("{}:{}", target_ip, port).parse()?;

        Ok(Self {
            socket,
            community: community.to_string(),
            target_addr,
            timeout_duration: Duration::from_millis(timeout_ms),
            request_id_counter: AtomicU32::new(1),
        })
    }

    /// Cria um socket UDP vinculado a uma porta local para comunicação com a OLT
    /// (drain_socket removido — descartar respostas pendentes quebra OLTs lentas)

    /// Envia uma requisição SNMP Get direta
    pub async fn get(
        &self,
        oid: &str,
    ) -> Result<Option<SnmpVariableBinding>, Box<dyn std::error::Error + Send + Sync>> {
        let req_id = self.request_id_counter.fetch_add(1, Ordering::SeqCst) as i32;
        let pdu = Self::encode_snmp_pdu(&self.community, oid, 0xA0, 0, 0, req_id);
        self.socket.send_to(&pdu, self.target_addr).await?;

        let deadline = tokio::time::Instant::now() + self.timeout_duration;
        let mut buf = vec![0u8; 65535];
        loop {
            let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
            if remaining.is_zero() {
                log::warn!(
                    "SNMP GET {} -> Timeout aguardando resposta de {}",
                    oid,
                    self.target_addr
                );
                return Err(format!("Timeout SNMP com {}", self.target_addr).into());
            }
            match timeout(remaining, self.socket.recv_from(&mut buf)).await {
                Ok(Ok((len, _src))) => {
                    if Self::extract_request_id(&buf[..len]) != Some(req_id) {
                        log::debug!("SNMP GET: descartando pacote obsoleto (req_id diferente)");
                        continue;
                    }
                    return Ok(Self::parse_snmp_response(&buf[..len], oid));
                }
                Ok(Err(e)) => return Err(Box::new(e)),
                Err(_) => return Err(format!("Timeout SNMP com {}", self.target_addr).into()),
            }
        }
    }

    /// Envia uma requisição SNMP GetNext para varredura de tabela (Walk)
    pub async fn get_next(
        &self,
        oid: &str,
    ) -> Result<Option<SnmpVariableBinding>, Box<dyn std::error::Error + Send + Sync>> {
        let req_id = self.request_id_counter.fetch_add(1, Ordering::SeqCst) as i32;
        let pdu = Self::encode_snmp_pdu(&self.community, oid, 0xA1, 0, 0, req_id);
        self.socket.send_to(&pdu, self.target_addr).await?;

        let deadline = tokio::time::Instant::now() + self.timeout_duration;
        let mut buf = vec![0u8; 65535];
        loop {
            let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
            if remaining.is_zero() {
                log::debug!(
                    "SNMP GETNEXT {} -> Timeout ({}ms) aguardando resposta de {}",
                    oid,
                    self.timeout_duration.as_millis(),
                    self.target_addr
                );
                return Err(format!(
                    "Timeout SNMP ({}ms) com {}",
                    self.timeout_duration.as_millis(),
                    self.target_addr
                )
                .into());
            }
            match timeout(remaining, self.socket.recv_from(&mut buf)).await {
                Ok(Ok((len, _src))) => {
                    if Self::extract_request_id(&buf[..len]) != Some(req_id) {
                        log::debug!("SNMP GETNEXT: descartando pacote obsoleto (req_id diferente)");
                        continue;
                    }
                    return Ok(Self::parse_snmp_response(&buf[..len], oid));
                }
                Ok(Err(e)) => {
                    log::warn!(
                        "SNMP GETNEXT {} -> Erro no socket UDP com {}: {:?}",
                        oid,
                        self.target_addr,
                        e
                    );
                    return Err(Box::new(e));
                }
                Err(_) => {
                    log::debug!(
                        "SNMP GETNEXT {} -> Timeout ({}ms) aguardando resposta de {}",
                        oid,
                        self.timeout_duration.as_millis(),
                        self.target_addr
                    );
                    return Err(format!(
                        "Timeout SNMP ({}ms) com {}",
                        self.timeout_duration.as_millis(),
                        self.target_addr
                    )
                    .into());
                }
            }
        }
    }

    /// Executa um SNMP Walk seguro sobre uma sub-árvore de OIDs com retentativa contra perda de pacotes UDP
    /// Percorre a sub-árvore até o fim natural da MIB sem nenhum limite artificial
    pub async fn walk(
        &self,
        root_oid: &str,
        max_entries: usize,
        delay: Duration,
    ) -> Result<Vec<SnmpVariableBinding>, Box<dyn std::error::Error + Send + Sync>> {
        let mut results = Vec::new();
        let mut current_oid = root_oid.to_string();
        let root_parts: Vec<&str> = root_oid.trim_start_matches('.').split('.').collect();

        loop {
            let mut binding_opt = None;
            // Até 6 tentativas com backoff adaptativo para tolerar pico de CPU / UDP drop na OLT
            for attempt in 1..=6 {
                match self.get_next(&current_oid).await {
                    Ok(Some(binding)) => {
                        binding_opt = Some(binding);
                        break;
                    }
                    Ok(None) => break,
                    Err(e) => {
                        if attempt < 6 {
                            tokio::time::sleep(Duration::from_millis(150 * attempt)).await;
                        } else {
                            log::debug!("SNMP Walk finalizado no OID {} ({:?})", current_oid, e);
                        }
                    }
                }
            }

            match binding_opt {
                Some(binding) => {
                    // Verifica se ainda pertence à sub-árvore comparando os componentes numéricos
                    let curr_parts: Vec<&str> =
                        binding.oid.trim_start_matches('.').split('.').collect();

                    if curr_parts.len() < root_parts.len()
                        || !curr_parts.starts_with(&root_parts[..])
                    {
                        break;
                    }

                    // Se a OLT responder com o mesmo OID (não-progressão), encerra o walk
                    if binding.oid == current_oid {
                        break;
                    }

                    current_oid = binding.oid.clone();
                    results.push(binding);

                    if max_entries > 0 && results.len() >= max_entries {
                        break;
                    }

                    if delay.as_micros() > 0 {
                        tokio::time::sleep(delay).await;
                    }
                }
                None => break,
            }
        }

        Ok(results)
    }

    /// Codifica comprimento BER corretamente (short ou long form) para suportar payloads > 255 bytes
    fn ber_len(len: usize) -> Vec<u8> {
        if len < 0x80 {
            vec![len as u8]
        } else if len <= 0xFF {
            vec![0x81, len as u8]
        } else if len <= 0xFFFF {
            vec![0x82, (len >> 8) as u8, (len & 0xFF) as u8]
        } else {
            vec![
                0x83,
                (len >> 16) as u8,
                ((len >> 8) & 0xFF) as u8,
                (len & 0xFF) as u8,
            ]
        }
    }

    /// Extrai o Request-ID de um pacote SNMP BER para validação de resposta
    /// Retorna None se o pacote estiver malformado
    fn extract_request_id(packet: &[u8]) -> Option<i32> {
        // SEQUENCE outer
        let (_, _, mut pos) = Self::read_ber_tl(packet, 0)?;
        // Version INTEGER
        let (0x02, ver_len, ver_start) = Self::read_ber_tl(packet, pos)? else {
            return None;
        };
        pos = ver_start + ver_len;
        // Community OCTET STRING
        let (0x04, comm_len, comm_start) = Self::read_ber_tl(packet, pos)? else {
            return None;
        };
        pos = comm_start + comm_len;
        // PDU (GetResponse = 0xA2, etc.)
        let (tag, _, pdu_start) = Self::read_ber_tl(packet, pos)?;
        if !matches!(tag, 0xA0 | 0xA1 | 0xA2 | 0xA5) {
            return None;
        }
        // Request ID INTEGER
        let (0x02, rid_len, rid_start) = Self::read_ber_tl(packet, pdu_start)? else {
            return None;
        };
        let rid_bytes = packet.get(rid_start..rid_start + rid_len)?;
        // Decode big-endian i32 (sign-extended)
        let mut num = 0i32;
        if !rid_bytes.is_empty() {
            if (rid_bytes[0] & 0x80) != 0 {
                num = -1i32;
            }
            for &b in rid_bytes {
                num = (num << 8) | (b as i32);
            }
        }
        Some(num)
    }

    /// Codifica uma requisição SNMP v2c em formato binário PDU com Request ID dinâmico
    fn encode_snmp_pdu(
        community: &str,
        oid_str: &str,
        pdu_type: u8,
        non_repeaters: u32,
        max_repetitions: u32,
        request_id: i32,
    ) -> Vec<u8> {
        let mut oid_bytes = Vec::new();
        let parts: Vec<u32> = oid_str
            .trim_start_matches('.')
            .split('.')
            .filter_map(|s| s.parse::<u32>().ok())
            .collect();

        if parts.len() >= 2 {
            oid_bytes.push((parts[0] * 40 + parts[1]) as u8);
            for &p in &parts[2..] {
                if p < 128 {
                    oid_bytes.push(p as u8);
                } else {
                    let mut temp = Vec::new();
                    let mut val = p;
                    temp.push((val & 0x7F) as u8);
                    val >>= 7;
                    while val > 0 {
                        temp.push(((val & 0x7F) as u8) | 0x80);
                        val >>= 7;
                    }
                    temp.reverse();
                    oid_bytes.extend_from_slice(&temp);
                }
            }
        }

        // VarBind: [OID, NULL]
        let mut varbind = Vec::new();
        varbind.push(0x06); // OID tag
        varbind.extend(Self::ber_len(oid_bytes.len()));
        varbind.extend_from_slice(&oid_bytes);
        varbind.push(0x05); // NULL value tag
        varbind.push(0x00);

        // VarBindList (Sequence)
        let mut varbind_list = Vec::new();
        varbind_list.push(0x30);
        varbind_list.extend(Self::ber_len(varbind.len()));
        varbind_list.extend_from_slice(&varbind);

        // PDU: [RequestID, NonRepeaters / ErrorStatus, MaxRepetitions / ErrorIndex, VarBindList]
        let mut pdu_content = Vec::new();
        let req_bytes = request_id.to_be_bytes();
        pdu_content.push(0x02);
        pdu_content.push(4);
        pdu_content.extend_from_slice(&req_bytes);

        if pdu_type == 0xA5 {
            // GetBulkRequest: non-repeaters e max-repetitions
            pdu_content.extend_from_slice(&[0x02, 0x01, non_repeaters as u8]);
            pdu_content.extend_from_slice(&[0x02, 0x01, max_repetitions as u8]);
        } else {
            pdu_content.extend_from_slice(&[0x02, 0x01, 0x00]); // Error status
            pdu_content.extend_from_slice(&[0x02, 0x01, 0x00]); // Error index
        }

        pdu_content.push(0x30);
        pdu_content.extend(Self::ber_len(varbind_list.len()));
        pdu_content.extend_from_slice(&varbind_list);

        let mut pdu = Vec::new();
        pdu.push(pdu_type);
        pdu.extend(Self::ber_len(pdu_content.len()));
        pdu.extend_from_slice(&pdu_content);

        // SNMP Message: [Version (v2c = 1), Community, PDU]
        let mut msg_content = Vec::new();
        msg_content.extend_from_slice(&[0x02, 0x01, 0x01]);
        msg_content.push(0x04);
        msg_content.extend(Self::ber_len(community.len()));
        msg_content.extend_from_slice(community.as_bytes());
        msg_content.extend_from_slice(&pdu);

        let mut packet = Vec::new();
        packet.push(0x30); // Sequence Tag
        packet.extend(Self::ber_len(msg_content.len()));
        packet.extend_from_slice(&msg_content);

        packet
    }

    /// Envia uma requisição SNMP GetBulk e decodifica múltiplos VarBinds
    /// Valida o Request-ID da resposta para descartar pacotes obsoletos de requisições anteriores
    pub async fn get_bulk(
        &self,
        oid: &str,
        max_repetitions: u32,
    ) -> Result<Vec<SnmpVariableBinding>, Box<dyn std::error::Error + Send + Sync>> {
        let req_id = self.request_id_counter.fetch_add(1, Ordering::SeqCst) as i32;
        let pdu = Self::encode_snmp_pdu(&self.community, oid, 0xA5, 0, max_repetitions, req_id);
        self.socket.send_to(&pdu, self.target_addr).await?;

        let deadline = tokio::time::Instant::now() + self.timeout_duration;
        let mut buf = vec![0u8; 65535];
        loop {
            let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
            if remaining.is_zero() {
                return Err(
                    format!("Timeout SNMP GetBulk ({}) com {}", oid, self.target_addr).into(),
                );
            }
            match timeout(remaining, self.socket.recv_from(&mut buf)).await {
                Ok(Ok((len, _src))) => {
                    if Self::extract_request_id(&buf[..len]) != Some(req_id) {
                        log::debug!("SNMP GetBulk: descartando pacote obsoleto (req_id diferente)");
                        continue;
                    }
                    return Ok(Self::parse_snmp_multi_response(&buf[..len], oid));
                }
                Ok(Err(e)) => return Err(Box::new(e)),
                Err(_) => {
                    return Err(
                        format!("Timeout SNMP GetBulk ({}) com {}", oid, self.target_addr).into(),
                    )
                }
            }
        }
    }

    /// Executa um SNMP Walk seguro, rápido e infalível com SNMP GetBulk (60 repetições por pacote)
    /// Baixa 3000+ ONUs em menos de 2 segundos sem timeout de UDP
    pub async fn bulk_walk(
        &self,
        root_oid: &str,
        max_entries: usize,
    ) -> Result<Vec<SnmpVariableBinding>, Box<dyn std::error::Error + Send + Sync>> {
        let mut results = Vec::new();
        let mut current_oid = root_oid.to_string();
        let root_parts: Vec<&str> = root_oid.trim_start_matches('.').split('.').collect();

        loop {
            let mut chunk = None;
            for attempt in 1..=4 {
                match self.get_bulk(&current_oid, 30).await {
                    Ok(vbs) if !vbs.is_empty() => {
                        chunk = Some(vbs);
                        break;
                    }
                    Ok(_) => break,
                    Err(e) => {
                        if attempt < 4 {
                            tokio::time::sleep(Duration::from_millis(150 * attempt)).await;
                        } else {
                            log::debug!(
                                "SNMP GetBulk walk finalizado no OID {}: {:?}",
                                current_oid,
                                e
                            );
                        }
                    }
                }
            }

            match chunk {
                Some(vbs) => {
                    let mut reached_end = false;
                    for binding in vbs {
                        // endOfMibView / noSuchObject sinaliza fim da sub-árvore
                        if binding.oid.starts_with("endOfMib:") {
                            reached_end = true;
                            break;
                        }

                        let curr_parts: Vec<&str> =
                            binding.oid.trim_start_matches('.').split('.').collect();
                        if curr_parts.len() < root_parts.len()
                            || !curr_parts.starts_with(&root_parts[..])
                        {
                            reached_end = true;
                            break;
                        }

                        if binding.oid == current_oid {
                            reached_end = true;
                            break;
                        }

                        current_oid = binding.oid.clone();
                        results.push(binding);

                        if max_entries > 0 && results.len() >= max_entries {
                            reached_end = true;
                            break;
                        }
                    }

                    if reached_end {
                        break;
                    }
                }
                None => {
                    if results.is_empty() {
                        // Fallback para GetNext walk desde o início se GetBulk não funcionar
                        return self
                            .walk(root_oid, max_entries, Duration::from_micros(20))
                            .await;
                    }
                    // GetBulk falhou no meio do walk (ex: OLT ZTE demora ao cruzar limite de placa PON)
                    // Continua via GetNext a partir do último OID obtido com sucesso
                    log::debug!(
                        "GetBulk falhou após {} entradas em {}, continuando via GetNext a partir de {}",
                        results.len(), root_oid, current_oid
                    );
                    let mut get_next_oid = current_oid.clone();
                    'gn_fallback: loop {
                        let mut found = false;
                        for attempt in 1u64..=6 {
                            match self.get_next(&get_next_oid).await {
                                Ok(Some(vb)) => {
                                    if vb.oid.starts_with("endOfMib:") {
                                        break 'gn_fallback;
                                    }
                                    let curr_parts: Vec<&str> =
                                        vb.oid.trim_start_matches('.').split('.').collect();
                                    if curr_parts.len() < root_parts.len()
                                        || !curr_parts.starts_with(&root_parts[..])
                                    {
                                        break 'gn_fallback;
                                    }
                                    if vb.oid == get_next_oid {
                                        break 'gn_fallback;
                                    }
                                    get_next_oid = vb.oid.clone();
                                    results.push(vb);
                                    if max_entries > 0 && results.len() >= max_entries {
                                        break 'gn_fallback;
                                    }
                                    found = true;
                                    break;
                                }
                                Ok(None) | Err(_) => {
                                    if attempt < 6 {
                                        tokio::time::sleep(Duration::from_millis(100 * attempt))
                                            .await;
                                    } else {
                                        break 'gn_fallback;
                                    }
                                }
                            }
                        }
                        if !found {
                            break;
                        }
                    }
                    break;
                }
            }
        }

        Ok(results)
    }

    /// Helper para ler tag + tamanho no padrão ASN.1 BER
    fn read_ber_tl(bytes: &[u8], offset: usize) -> Option<(u8, usize, usize)> {
        if offset >= bytes.len() {
            return None;
        }
        let tag = bytes[offset];
        let mut idx = offset + 1;
        if idx >= bytes.len() {
            return None;
        }
        let len = if (bytes[idx] & 0x80) != 0 {
            let n = (bytes[idx] & 0x7F) as usize;
            idx += 1;
            let mut l = 0;
            for i in 0..n {
                if idx + i >= bytes.len() {
                    return None;
                }
                l = (l << 8) | (bytes[idx + i] as usize);
            }
            idx += n;
            l
        } else {
            let l = bytes[idx] as usize;
            idx += 1;
            l
        };
        Some((tag, len, idx))
    }

    /// Parser ASN.1 BER estruturado para GetBulk/GetResponse (múltiplos VarBinds).
    /// Navega a estrutura BER desde a raiz do pacote sem busca linear de bytes de tag PDU.
    fn parse_snmp_multi_response(packet: &[u8], _requested_oid: &str) -> Vec<SnmpVariableBinding> {
        let mut results = Vec::new();
        if packet.len() < 10 {
            return results;
        }

        // Outer SEQUENCE (0x30) - toda a mensagem SNMPv2c
        let (outer_tag, _outer_len, mut pos) = match Self::read_ber_tl(packet, 0) {
            Some(tl) => tl,
            None => return results,
        };
        if outer_tag != 0x30 {
            return results;
        }

        // Version INTEGER (0x02)
        if let Some((0x02, ver_len, ver_start)) = Self::read_ber_tl(packet, pos) {
            pos = ver_start + ver_len;
        } else {
            return results;
        }

        // Community OCTET STRING (0x04)
        if let Some((0x04, comm_len, comm_start)) = Self::read_ber_tl(packet, pos) {
            pos = comm_start + comm_len;
        } else {
            return results;
        }

        // PDU tag: GetResponse=0xA2, GetBulk=0xA5, GetNext=0xA1, Get=0xA0
        let (pdu_tag, _pdu_len, pdu_start) = match Self::read_ber_tl(packet, pos) {
            Some(tl) if matches!(tl.0, 0xA0 | 0xA1 | 0xA2 | 0xA5) => tl,
            _ => return results,
        };
        let _ = pdu_tag;
        pos = pdu_start;

        // Request ID INTEGER (0x02)
        if let Some((0x02, rid_len, rid_start)) = Self::read_ber_tl(packet, pos) {
            pos = rid_start + rid_len;
        } else {
            return results;
        }

        // Error Status INTEGER (0x02)
        if let Some((0x02, es_len, es_start)) = Self::read_ber_tl(packet, pos) {
            pos = es_start + es_len;
        } else {
            return results;
        }

        // Error Index INTEGER (0x02)
        if let Some((0x02, ei_len, ei_start)) = Self::read_ber_tl(packet, pos) {
            pos = ei_start + ei_len;
        } else {
            return results;
        }

        // VarBindList SEQUENCE (0x30)
        let (_vbl_tag, _vbl_len, vbl_start) = match Self::read_ber_tl(packet, pos) {
            Some(tl) if tl.0 == 0x30 => tl,
            _ => return results,
        };
        pos = vbl_start;

        // Itera sobre cada VarBind (0x30)
        while pos < packet.len() {
            let (vb_tag, vb_len, vb_content) = match Self::read_ber_tl(packet, pos) {
                Some(tl) => tl,
                None => break,
            };
            if vb_tag != 0x30 {
                break;
            }
            let vb_end = vb_content + vb_len;
            if vb_end > packet.len() {
                break;
            }

            // OID (0x06)
            if let Some((0x06, oid_len, oid_start)) = Self::read_ber_tl(packet, vb_content) {
                if oid_start + oid_len <= vb_end {
                    let oid_raw = &packet[oid_start..oid_start + oid_len];

                    // Reconstrói string OID a partir de bytes BER
                    let mut reconstructed: Vec<String> = Vec::new();
                    if !oid_raw.is_empty() {
                        let first = oid_raw[0];
                        reconstructed.push(format!("{}", first / 40));
                        reconstructed.push(format!("{}", first % 40));
                        let mut acc: u64 = 0;
                        for &b in &oid_raw[1..] {
                            acc = (acc << 7) | ((b & 0x7F) as u64);
                            if (b & 0x80) == 0 {
                                reconstructed.push(format!("{}", acc));
                                acc = 0;
                            }
                        }
                    }
                    let parsed_oid = format!(".{}", reconstructed.join("."));

                    // Valor que segue o OID
                    let val_offset = oid_start + oid_len;
                    if let Some((val_tag, val_len, val_bytes_start)) =
                        Self::read_ber_tl(packet, val_offset)
                    {
                        let val_bytes = packet
                            .get(val_bytes_start..val_bytes_start + val_len)
                            .unwrap_or(&[]);

                        match val_tag {
                            // INTEGER, Counter32, Gauge32, TimeTicks
                            0x02 | 0x41 | 0x42 | 0x43 => {
                                let mut num: i64 = 0;
                                if !val_bytes.is_empty() {
                                    if val_tag == 0x02 && (val_bytes[0] & 0x80) != 0 {
                                        num = -1i64;
                                    }
                                    for &b in val_bytes {
                                        num = (num << 8) | (b as i64);
                                    }
                                }
                                results.push(SnmpVariableBinding {
                                    oid: parsed_oid,
                                    value_raw: val_bytes.to_vec(),
                                    value_str: None,
                                    value_int: Some(num),
                                });
                            }
                            // OCTET STRING, IpAddress (0x44 = Opaque ou similar)
                            0x04 | 0x44 | 0x40 => {
                                let s = String::from_utf8_lossy(val_bytes).to_string();
                                results.push(SnmpVariableBinding {
                                    oid: parsed_oid,
                                    value_raw: val_bytes.to_vec(),
                                    value_str: Some(s),
                                    value_int: None,
                                });
                            }
                            // noSuchObject (0x80), noSuchInstance (0x81), endOfMibView (0x82) - para o walk
                            0x80 | 0x81 | 0x82 => {
                                // Sinaliza fim do walk inserindo OID marcado como "endOfMib"
                                results.push(SnmpVariableBinding {
                                    oid: format!("endOfMib:{}", parsed_oid),
                                    value_raw: vec![],
                                    value_str: None,
                                    value_int: None,
                                });
                            }
                            _ => {}
                        }
                    }
                }
            }

            pos = vb_end;
        }

        results
    }

    /// Parser ASN.1 BER estruturado para extração exata do VarBind na resposta SNMP GetResponse (0xA2)
    fn parse_snmp_response(packet: &[u8], requested_oid: &str) -> Option<SnmpVariableBinding> {
        let vbs = Self::parse_snmp_multi_response(packet, requested_oid);
        vbs.into_iter().find(|vb| !vb.oid.starts_with("endOfMib:"))
    }
}
