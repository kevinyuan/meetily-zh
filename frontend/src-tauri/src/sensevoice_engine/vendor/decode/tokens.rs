use std::collections::HashMap;
use std::fs;
use std::path::Path;

/// Load a vocabulary file where each line is `token id`.
///
/// Returns a Vec indexed by token ID, and the blank token index.
/// Replaces `▁` (U+2581) with space in token strings.
pub fn load_vocab(path: &Path) -> Result<(Vec<String>, Option<i32>), std::io::Error> {
    let content = fs::read_to_string(path)?;

    let mut max_id = 0;
    let mut tokens_with_ids: Vec<(String, usize)> = Vec::new();
    let mut blank_idx: Option<i32> = None;

    for line in content.lines() {
        let parts: Vec<&str> = line.trim_end().split(' ').collect();
        if parts.len() >= 2 {
            let token = parts[0].to_string();
            if let Ok(id) = parts[1].parse::<usize>() {
                if token == "<blk>" {
                    blank_idx = Some(id as i32);
                }
                tokens_with_ids.push((token, id));
                max_id = max_id.max(id);
            }
        }
    }

    let mut vocab = vec![String::new(); max_id + 1];
    for (token, id) in tokens_with_ids {
        vocab[id] = token.replace('\u{2581}', " ");
    }

    log::info!("Loaded {} vocab tokens from {:?}", vocab.len(), path);
    Ok((vocab, blank_idx))
}

/// Symbol table mapping token IDs to strings.
///
/// Supports two file formats:
/// - `symbol id` (split on last whitespace, used by SenseVoice)
/// - Optionally base64-encoded symbols (for FunASR Nano models)
pub struct SymbolTable {
    id_to_sym: HashMap<i64, String>,
}

impl SymbolTable {
    /// Load a symbol table from a file where each line is `symbol id`.
    pub fn load(path: &Path) -> Result<Self, std::io::Error> {
        let contents = fs::read_to_string(path)?;
        let mut id_to_sym = HashMap::new();

        for line in contents.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }

            let parts: Vec<&str> = line.rsplitn(2, |c: char| c.is_whitespace()).collect();
            if parts.len() == 2 {
                if let Ok(id) = parts[0].parse::<i64>() {
                    id_to_sym.insert(id, parts[1].to_string());
                }
            }
        }

        log::info!("Loaded {} tokens from {:?}", id_to_sym.len(), path);
        Ok(Self { id_to_sym })
    }

    /// Decode all symbols from base64 (for FunASR Nano models).
    pub fn apply_base64_decode(&mut self) {
        use base64::{engine::general_purpose::STANDARD, Engine as _};
        for sym in self.id_to_sym.values_mut() {
            if let Ok(bytes) = STANDARD.decode(sym.as_bytes()) {
                if let Ok(decoded) = String::from_utf8(bytes) {
                    *sym = decoded;
                }
            }
        }
    }

    pub fn get(&self, id: i64) -> Option<&str> {
        self.id_to_sym.get(&id).map(|s| s.as_str())
    }

    pub fn get_or_empty(&self, id: i64) -> &str {
        self.id_to_sym.get(&id).map(|s| s.as_str()).unwrap_or("")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn write_tmp(contents: &str) -> tempfile::NamedTempFile {
        let mut file = tempfile::NamedTempFile::new().unwrap();
        file.write_all(contents.as_bytes()).unwrap();
        file
    }

    #[test]
    fn load_vocab_indexes_by_id_and_finds_the_blank() {
        let file = write_tmp("<blk> 0\n你好 1\n\u{2581}the 2\n");
        let (vocab, blank) = load_vocab(file.path()).unwrap();
        assert_eq!(blank, Some(0));
        assert_eq!(vocab[1], "你好");
        assert_eq!(vocab[2], " the", "the sentencepiece marker becomes a space");
    }

    #[test]
    fn load_vocab_without_a_blank_reports_none() {
        let file = write_tmp("a 0\nb 1\n");
        assert_eq!(load_vocab(file.path()).unwrap().1, None);
    }

    #[test]
    fn symbol_table_splits_on_the_last_whitespace() {
        // Symbols can themselves contain spaces; the id is always the last field.
        let file = write_tmp("hello world 7\n你 8\n\n");
        let table = SymbolTable::load(file.path()).unwrap();
        assert_eq!(table.get(7), Some("hello world"));
        assert_eq!(table.get(8), Some("你"));
    }

    #[test]
    fn symbol_table_ignores_malformed_lines() {
        let file = write_tmp("ok 1\nno-id-here\nbad abc\n");
        let table = SymbolTable::load(file.path()).unwrap();
        assert_eq!(table.get(1), Some("ok"));
        assert_eq!(table.get_or_empty(2), "");
        assert_eq!(table.get(2), None);
    }

    #[test]
    fn base64_symbols_are_decoded_in_place() {
        // "你好" in base64.
        let file = write_tmp("5L2g5aW9 3\n");
        let mut table = SymbolTable::load(file.path()).unwrap();
        assert_eq!(table.get(3), Some("5L2g5aW9"));
        table.apply_base64_decode();
        assert_eq!(table.get(3), Some("你好"));
    }
}
