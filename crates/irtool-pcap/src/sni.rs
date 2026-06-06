/// 从 TLS ClientHello 中提取 SNI 域名
/// 输入是 TCP payload（不含 IP/TCP 头）
pub fn extract_sni(payload: &[u8]) -> Option<String> {
    // TLS Record: ContentType(1) + Version(2) + Length(2)
    if payload.len() < 5 { return None; }
    if payload[0] != 0x16 { return None; } // Not Handshake
    // Skip TLS record header
    let record_len = ((payload[3] as usize) << 8) | (payload[4] as usize);
    if payload.len() < 5 + record_len { return None; }

    let handshake = &payload[5..];
    // Handshake: Type(1) + Length(3)
    if handshake.len() < 4 { return None; }
    if handshake[0] != 0x01 { return None; } // Not ClientHello

    let client_hello = &handshake[4..];
    // ClientHello: Version(2) + Random(32) + SessionIDLen(1)
    if client_hello.len() < 34 { return None; }
    let mut offset = 2 + 32;

    // Session ID
    let session_id_len = client_hello[offset] as usize;
    offset += 1 + session_id_len;
    if client_hello.len() < offset { return None; }

    // Cipher Suites
    if client_hello.len() < offset + 2 { return None; }
    let cipher_suites_len = ((client_hello[offset] as usize) << 8) | (client_hello[offset + 1] as usize);
    offset += 2 + cipher_suites_len;
    if client_hello.len() < offset { return None; }

    // Compression Methods
    if client_hello.len() < offset + 1 { return None; }
    let compression_len = client_hello[offset] as usize;
    offset += 1 + compression_len;
    if client_hello.len() < offset + 2 { return None; }

    // Extensions
    let extensions_len = ((client_hello[offset] as usize) << 8) | (client_hello[offset + 1] as usize);
    offset += 2;
    if client_hello.len() < offset + extensions_len { return None; }

    let extensions = &client_hello[offset..offset + extensions_len];
    let mut ext_offset = 0;

    while ext_offset + 4 <= extensions.len() {
        let ext_type = ((extensions[ext_offset] as u16) << 8) | (extensions[ext_offset + 1] as u16);
        let ext_len = ((extensions[ext_offset + 2] as usize) << 8) | (extensions[ext_offset + 3] as usize);
        ext_offset += 4;

        if ext_type == 0x0000 { // SNI extension
            if ext_len < 2 { return None; }
            // Server Name List Length(2)
            let list_len = ((extensions[ext_offset] as usize) << 8) | (extensions[ext_offset + 1] as usize);
            let mut name_offset = ext_offset + 2;
            let list_end = ext_offset + 2 + list_len;

            while name_offset + 3 <= list_end && name_offset < extensions.len() {
                let name_type = extensions[name_offset];
                let name_len = ((extensions[name_offset + 1] as usize) << 8) | (extensions[name_offset + 2] as usize);
                name_offset += 3;

                if name_type == 0x00 { // host_name
                    if name_offset + name_len <= extensions.len() {
                        return std::str::from_utf8(&extensions[name_offset..name_offset + name_len])
                            .ok()
                            .map(|s| s.to_string());
                    }
                }
                name_offset += name_len;
            }
        }
        ext_offset += ext_len;
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_sni_from_client_hello() {
        let mut payload = vec![
            0x16, // ContentType: Handshake
            0x03, 0x01, // Version: TLS 1.0
            0x00, 0x00, // Length (will fill in)
            0x01, // HandshakeType: ClientHello
            0x00, 0x00, 0x00, // Length (will fill in)
            0x03, 0x03, // ClientHello Version: TLS 1.2
        ];
        // Random (32 bytes)
        payload.extend_from_slice(&[0u8; 32]);
        // Session ID length: 0
        payload.push(0x00);
        // Cipher Suites length: 2
        payload.extend_from_slice(&[0x00, 0x02]);
        // Cipher Suite: TLS_RSA_WITH_AES_128_CBC_SHA
        payload.extend_from_slice(&[0x00, 0x2F]);
        // Compression Methods length: 1
        payload.push(0x01);
        // Compression Method: null
        payload.push(0x00);
        // Extensions length (will fill in)
        let ext_len_offset = payload.len();
        payload.extend_from_slice(&[0x00, 0x00]);
        // SNI Extension
        let domain = b"example.com";
        payload.extend_from_slice(&[0x00, 0x00]); // Extension type: SNI
        let sni_ext_len = (2 + 1 + 2 + domain.len()) as u16; // list_len(2) + type(1) + len(2) + name
        payload.extend_from_slice(&[(sni_ext_len >> 8) as u8, (sni_ext_len & 0xFF) as u8]);
        // Server Name List Length
        let list_len = (1 + 2 + domain.len()) as u16;
        payload.extend_from_slice(&[(list_len >> 8) as u8, (list_len & 0xFF) as u8]);
        // Server Name Type: host_name
        payload.push(0x00);
        // Server Name Length
        payload.extend_from_slice(&[(domain.len() >> 8) as u8, (domain.len() & 0xFF) as u8]);
        // Server Name
        payload.extend_from_slice(domain);

        // Fill in lengths
        let total_after_record_header = payload.len() - 5;
        payload[3] = ((total_after_record_header >> 8) & 0xFF) as u8;
        payload[4] = (total_after_record_header & 0xFF) as u8;

        let handshake_body_len = total_after_record_header - 4;
        payload[6] = ((handshake_body_len >> 16) & 0xFF) as u8;
        payload[7] = ((handshake_body_len >> 8) & 0xFF) as u8;
        payload[8] = (handshake_body_len & 0xFF) as u8;

        let ext_total_len = payload.len() - ext_len_offset - 2;
        payload[ext_len_offset] = ((ext_total_len >> 8) & 0xFF) as u8;
        payload[ext_len_offset + 1] = (ext_total_len & 0xFF) as u8;

        let result = extract_sni(&payload);
        assert_eq!(result, Some("example.com".to_string()));
    }
}
