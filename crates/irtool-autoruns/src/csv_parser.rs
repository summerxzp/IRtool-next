use irtool_core::IrError;

pub struct RawEntry {
    pub location: String,
    pub entry: String,
    pub enabled: String,
    pub category: String,
    pub description: String,
    pub publisher: String,
    pub image_path: String,
    pub launch_string: String,
    pub timestamp: String,
    pub md5: String,
    pub sha256: String,
    pub signer: String,
    pub version: String,
}

pub fn decode_bytes(raw: &[u8]) -> String {
    if raw.len() >= 2 && raw[0] == 0xFF && raw[1] == 0xFE {
        return encoding_rs::UTF_16LE.decode(raw).0.into_owned();
    }
    if raw.len() >= 3 && raw[0] == 0xEF && raw[1] == 0xBB && raw[2] == 0xBF {
        return encoding_rs::UTF_8.decode(raw).0.into_owned();
    }
    if let Ok(s) = std::str::from_utf8(raw) {
        return s.to_owned();
    }
    let null_count = raw.iter().take(1024).filter(|&&b| b == 0).count();
    if null_count > raw.len().min(1024) / 4 {
        return encoding_rs::UTF_16LE.decode(raw).0.into_owned();
    }
    encoding_rs::GBK.decode(raw).0.into_owned()
}

pub fn parse(raw_bytes: &[u8]) -> Result<Vec<RawEntry>, IrError> {
    let csv_text = decode_bytes(raw_bytes);
    let csv_text = csv_text.trim_start_matches('\u{feff}');

    let mut reader = csv::ReaderBuilder::new()
        .flexible(true)
        .has_headers(true)
        .from_reader(csv_text.as_bytes());

    let headers = reader
        .headers()
        .map_err(|e| IrError::Parse(format!("CSV header read error: {}", e)))?
        .clone();

    let has_entry_location = headers.iter().any(|h| h == "Entry Location");
    if !has_entry_location {
        return Err(IrError::Parse("CSV missing 'Entry Location' column".into()));
    }

    let mut entries = Vec::new();
    for result in reader.records() {
        match result {
            Ok(record) => {
                let entry = build_raw_entry(&headers, &record);
                entries.push(entry);
            }
            Err(e) => {
                tracing::warn!("skipping malformed CSV row: {}", e);
            }
        }
    }

    Ok(entries)
}

fn get_field(headers: &csv::StringRecord, record: &csv::StringRecord, name: &str) -> String {
    headers
        .iter()
        .position(|h| h == name)
        .and_then(|i| record.get(i))
        .unwrap_or("")
        .trim()
        .to_owned()
}

fn build_raw_entry(headers: &csv::StringRecord, record: &csv::StringRecord) -> RawEntry {
    RawEntry {
        location: get_field(headers, record, "Entry Location"),
        entry: get_field(headers, record, "Entry"),
        enabled: get_field(headers, record, "Enabled"),
        category: get_field(headers, record, "Category"),
        description: get_field(headers, record, "Description"),
        publisher: get_field(headers, record, "Company"),
        image_path: get_field(headers, record, "Image Path"),
        launch_string: get_field(headers, record, "Launch String"),
        timestamp: get_field(headers, record, "Time"),
        md5: get_field(headers, record, "MD5"),
        sha256: get_field(headers, record, "SHA-256"),
        signer: get_field(headers, record, "Signer"),
        version: get_field(headers, record, "Version"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decode_utf16le_bom() {
        let input = "Entry Location,Entry\r\nHKLM\\Run,Test";
        let mut encoded = vec![0xFF, 0xFE];
        for ch in input.encode_utf16() {
            encoded.push(ch as u8);
            encoded.push((ch >> 8) as u8);
        }
        let decoded = decode_bytes(&encoded);
        assert!(decoded.contains("Entry Location"));
    }

    #[test]
    fn decode_utf8_plain() {
        let input = b"Entry Location,Entry\r\nHKLM\\Run,Test";
        let decoded = decode_bytes(input);
        assert!(decoded.contains("Entry Location"));
    }

    #[test]
    fn parse_simple_csv() {
        let csv = "Entry Location,Entry,Enabled,Category,Description,Company,Image Path,Launch String,Time,MD5,SHA-256,Signer,Version\r\n\
                   HKLM\\SOFTWARE\\Run,TestApp,enabled,Logon,Test App,TestCo,C:\\test.exe,C:\\test.exe,2024-01-01,,,TestCo (Verified),1.0\r\n";
        let entries = parse(csv.as_bytes()).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].entry, "TestApp");
        assert_eq!(entries[0].category, "Logon");
        assert_eq!(entries[0].image_path, "C:\\test.exe");
    }

    #[test]
    fn parse_missing_column_returns_error() {
        let csv = "Foo,Bar\r\n1,2\r\n";
        let result = parse(csv.as_bytes());
        assert!(result.is_err());
    }

    #[test]
    fn parse_flexible_tolerates_extra_fields() {
        let csv = "Entry Location,Entry,Enabled,Category,Description,Company,Image Path,Launch String,Time,MD5,SHA-256,Signer,Version\r\n\
                   HKLM\\Run,Test,enabled,Logon,D,D,D,D,D,D,D,D,D,EXTRA\r\n";
        let entries = parse(csv.as_bytes()).unwrap();
        assert_eq!(entries.len(), 1);
    }
}
