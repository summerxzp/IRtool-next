/// 从 DNS 报文中提取查询域名
/// 输入是 UDP payload（不含 IP/UDP 头）
pub fn extract_dns_query(payload: &[u8]) -> Option<DnsQueryInfo> {
    // DNS header: 12 bytes minimum
    if payload.len() < 12 { return None; }

    let flags = ((payload[2] as u16) << 8) | (payload[3] as u16);
    let qr = (flags >> 15) & 1;
    let opcode = (flags >> 11) & 0xF;

    // Only process queries (QR=0, Opcode=0 standard query)
    if qr != 0 || opcode != 0 { return None; }

    let qdcount = ((payload[4] as u16) << 8) | (payload[5] as u16);
    if qdcount == 0 { return None; }

    // Parse first question
    let mut offset = 12;
    let domain = parse_domain_name(payload, &mut offset)?;

    if offset + 4 > payload.len() { return None; }
    let qtype = ((payload[offset] as u16) << 8) | (payload[offset + 1] as u16);

    Some(DnsQueryInfo {
        domain,
        query_type: dns_type_to_string(qtype),
    })
}

#[derive(Debug, Clone)]
pub struct DnsQueryInfo {
    pub domain: String,
    pub query_type: String,
}

fn parse_domain_name(payload: &[u8], offset: &mut usize) -> Option<String> {
    let mut labels = Vec::new();
    let mut jumped = false;
    let mut jump_offset = 0;
    let max_jumps = 10;
    let mut jumps = 0;

    loop {
        if *offset >= payload.len() { return None; }
        let len = payload[*offset] as usize;

        if len == 0 {
            *offset += 1;
            break;
        }

        // Check for compression pointer (top 2 bits = 11)
        if (len & 0xC0) == 0xC0 {
            if *offset + 1 >= payload.len() { return None; }
            if !jumped {
                jump_offset = *offset + 2;
            }
            jumps += 1;
            if jumps > max_jumps { return None; }
            *offset = ((len & 0x3F) << 8) | (payload[*offset + 1] as usize);
            jumped = true;
            continue;
        }

        *offset += 1;
        if *offset + len > payload.len() { return None; }
        let label = std::str::from_utf8(&payload[*offset..*offset + len]).ok()?;
        labels.push(label.to_string());
        *offset += len;
    }

    if jumped {
        *offset = jump_offset;
    }

    Some(labels.join("."))
}

fn dns_type_to_string(qtype: u16) -> String {
    match qtype {
        1 => "A".to_string(),
        2 => "NS".to_string(),
        5 => "CNAME".to_string(),
        6 => "SOA".to_string(),
        12 => "PTR".to_string(),
        15 => "MX".to_string(),
        16 => "TXT".to_string(),
        28 => "AAAA".to_string(),
        33 => "SRV".to_string(),
        255 => "ANY".to_string(),
        _ => format!("TYPE{}", qtype),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_dns_query() {
        // Construct a minimal DNS query for "example.com" type A
        let mut packet = vec![
            0x00, 0x01, // ID
            0x01, 0x00, // Flags: standard query
            0x00, 0x01, // QDCOUNT: 1
            0x00, 0x00, // ANCOUNT: 0
            0x00, 0x00, // NSCOUNT: 0
            0x00, 0x00, // ARCOUNT: 0
        ];
        // QNAME: example.com
        packet.push(7); // "example" length
        packet.extend_from_slice(b"example");
        packet.push(3); // "com" length
        packet.extend_from_slice(b"com");
        packet.push(0); // null terminator
        // QTYPE: A (1)
        packet.extend_from_slice(&[0x00, 0x01]);
        // QCLASS: IN (1)
        packet.extend_from_slice(&[0x00, 0x01]);

        let result = extract_dns_query(&packet);
        assert!(result.is_some());
        let info = result.unwrap();
        assert_eq!(info.domain, "example.com");
        assert_eq!(info.query_type, "A");
    }
}
