//! 用户自定义 PII 词典加载（`.attune/pii_dict.yaml`）。
//!
//! ## 文件格式
//!
//! ```yaml
//! entries:
//!   - name: project_codename
//!     literals: [Apollo, Hermes, Zeus]
//!   - name: internal_alias
//!     regex: "C 部 [A-Z] 总"
//!   - name: employee_id
//!     regex: "E\\d{4}"
//! ```
//!
//! `literals` 与 `regex` 可同时存在；`name` 决定 placeholder 前缀
//! (如 `project_codename` → `[PROJECT_CODENAME_1]`)。

use regex::Regex;
use serde::Deserialize;
use std::path::Path;

#[derive(Debug, Deserialize)]
struct DictFile {
    entries: Vec<DictEntryRaw>,
}

#[derive(Debug, Deserialize)]
struct DictEntryRaw {
    name: String,
    #[serde(default)]
    literals: Vec<String>,
    #[serde(default)]
    regex: Option<String>,
}

#[derive(Debug)]
pub struct DictEntry {
    pub name: String,
    literals: Vec<String>,
    compiled_regex: Option<Regex>,
}

impl DictEntry {
    /// 仅含字面量的词典项。
    pub fn from_literals(name: impl Into<String>, literals: Vec<String>) -> Self {
        Self {
            name: name.into(),
            literals,
            compiled_regex: None,
        }
    }

    /// 仅含正则的词典项。正则编译失败返回 InvalidData 错误。
    pub fn from_regex(name: impl Into<String>, regex: &str) -> std::io::Result<Self> {
        let name = name.into();
        let re = Regex::new(regex).map_err(|e| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("invalid regex for entry '{name}': {e}"),
            )
        })?;
        Ok(Self {
            name,
            literals: Vec::new(),
            compiled_regex: Some(re),
        })
    }

    /// 在文本中找出所有命中区间（先字面量后正则，按出现位置）。
    pub fn find_all(&self, text: &str) -> Vec<(usize, usize)> {
        let mut out = Vec::new();
        for lit in &self.literals {
            if lit.is_empty() {
                continue;
            }
            let mut pos = 0;
            while let Some(idx) = text[pos..].find(lit) {
                let abs = pos + idx;
                out.push((abs, abs + lit.len()));
                pos = abs + lit.len();
            }
        }
        if let Some(re) = &self.compiled_regex {
            for m in re.find_iter(text) {
                out.push((m.start(), m.end()));
            }
        }
        out
    }
}

pub fn load(path: &Path) -> std::io::Result<Vec<DictEntry>> {
    let raw = std::fs::read_to_string(path)?;
    let file: DictFile = serde_yaml::from_str(&raw).map_err(|e| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("parse pii_dict.yaml: {e}"),
        )
    })?;
    let mut out = Vec::with_capacity(file.entries.len());
    for raw_entry in file.entries {
        let compiled_regex = match raw_entry.regex.as_deref() {
            Some(pattern) if !pattern.is_empty() => Some(Regex::new(pattern).map_err(|e| {
                std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!("invalid regex for entry '{}': {e}", raw_entry.name),
                )
            })?),
            _ => None,
        };
        out.push(DictEntry {
            name: raw_entry.name,
            literals: raw_entry.literals,
            compiled_regex,
        });
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    fn write_yaml(content: &str) -> NamedTempFile {
        let mut f = NamedTempFile::new().unwrap();
        f.write_all(content.as_bytes()).unwrap();
        f
    }

    #[test]
    fn literal_match() {
        let f = write_yaml(
            r#"
entries:
  - name: project_codename
    literals: [Apollo, Hermes]
"#,
        );
        let entries = load(f.path()).unwrap();
        assert_eq!(entries.len(), 1);
        let hits = entries[0].find_all("project Apollo and project Hermes start");
        assert_eq!(hits.len(), 2);
    }

    #[test]
    fn regex_match() {
        let f = write_yaml(
            r#"
entries:
  - name: employee_id
    regex: "E\\d{4}"
"#,
        );
        let entries = load(f.path()).unwrap();
        let hits = entries[0].find_all("employee E1234 and E5678 here");
        assert_eq!(hits.len(), 2);
    }

    #[test]
    fn both_literal_and_regex() {
        let f = write_yaml(
            r#"
entries:
  - name: secret
    literals: [ProjectX]
    regex: "TOKEN-\\w{4}"
"#,
        );
        let entries = load(f.path()).unwrap();
        let hits = entries[0].find_all("ProjectX uses TOKEN-AB12 daily");
        assert_eq!(hits.len(), 2);
    }

    #[test]
    fn invalid_regex_returns_error() {
        let f = write_yaml(
            r#"
entries:
  - name: bad
    regex: "[unclosed"
"#,
        );
        let err = load(f.path()).unwrap_err();
        assert!(err.to_string().contains("invalid regex"));
    }
}
