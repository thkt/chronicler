use sha2::{Digest, Sha256};

pub fn content_hash(content: &[u8]) -> String {
    let hash = Sha256::digest(content);
    format!("sha256:{:x}", hash)
}

#[cfg(test)]
mod tests {
    use super::*;

    // T-003: 同一内容 → 毎回同じSHA256値
    #[test]
    fn same_content_produces_same_hash() {
        let content = b"fn test_eval() { assert!(true); }";
        let hash1 = content_hash(content);
        let hash2 = content_hash(content);
        assert_eq!(hash1, hash2);
        assert!(hash1.starts_with("sha256:"));
    }

    // T-004: ファイル内容変更 → 異なるSHA256値
    #[test]
    fn different_content_produces_different_hash() {
        let hash1 = content_hash(b"version 1");
        let hash2 = content_hash(b"version 2");
        assert_ne!(hash1, hash2);
    }
}
