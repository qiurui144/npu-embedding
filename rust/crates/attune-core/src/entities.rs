//! 实体抽取：从中文 / 英文文本中抽出 Person / Money / Date / CaseNo / Company 等结构化实体。
//!
//! Sprint 1 Phase B 将使用这些实体计算 Project 推荐归类的"实体重叠度"
//! （spec §2.3 的 0.6 阈值）。
//!
//! 设计：纯函数 + 正则 + 中文启发式。无外部 API、无模型推理。

use regex::Regex;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EntityKind {
    Person,  // 中文 2-4 字人名
    Money,   // ¥xxx / 人民币 X 元 / 数额单位
    Date,    // YYYY-MM-DD / YYYY 年 M 月 D 日
    CaseNo,  // (YYYY)XX民终/民初/刑初 NNNN 号
    Company, // 含"有限公司"/"股份公司"/"研究所"等后缀
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Entity {
    pub kind: EntityKind,
    pub value: String,
    /// 在原文中的字节起止（UTF-8 byte offset）— 上层可截取上下文
    pub byte_start: usize,
    pub byte_end: usize,
}

/// 从给定文本抽出所有实体。返回顺序：按出现位置升序。
///
/// 对中文 + 英文混合文本鲁棒。空文本返回空 Vec。
pub fn extract_entities(text: &str) -> Vec<Entity> {
    if text.is_empty() {
        return Vec::new();
    }
    let mut out = Vec::new();

    extract_money(text, &mut out);
    extract_dates(text, &mut out);
    extract_case_no(text, &mut out);
    extract_company(text, &mut out);
    extract_chinese_person(text, &mut out);

    out.sort_by_key(|e| e.byte_start);
    out
}

fn push(out: &mut Vec<Entity>, kind: EntityKind, m: regex::Match<'_>) {
    out.push(Entity {
        kind,
        value: m.as_str().to_string(),
        byte_start: m.start(),
        byte_end: m.end(),
    });
}

fn extract_money(text: &str, out: &mut Vec<Entity>) {
    // ¥xxx / ¥xxx.xx / xxx 元 / 人民币 xxx 元 / 壹拾万元 等
    static MONEY_RE_PAT: &str = r"(?x)
        ( ¥ \s* \d[\d,]*(?:\.\d+)?
        | (?:人民币|RMB|CNY) \s* \d[\d,]*(?:\.\d+)? \s*(?:元|圆|万|亿)?
        | \d[\d,]*(?:\.\d+)? \s* (?:元|圆|万元|亿元|万|亿)
        | (?:壹|贰|叁|肆|伍|陆|柒|捌|玖|拾|佰|仟|万|亿|零|整|圆|元|角|分){2,}
        )
    ";
    let re = Regex::new(MONEY_RE_PAT).expect("money regex compile");
    for m in re.find_iter(text) {
        push(out, EntityKind::Money, m);
    }
}

fn extract_dates(text: &str, out: &mut Vec<Entity>) {
    // 2024-03-15 / 2024/3/15 / 2024 年 3 月 15 日
    // 顺序：先长 pattern（年月日），再短 pattern（年月 / ISO），后面被前者覆盖的丢弃。
    static DATE_PATS: &[&str] = &[
        r"\d{4}\s*年\s*\d{1,2}\s*月\s*\d{1,2}\s*日",
        r"\b\d{4}[-/]\d{1,2}[-/]\d{1,2}\b",
        r"\d{4}\s*年\s*\d{1,2}\s*月",
    ];
    let mut spans: Vec<(usize, usize, String)> = Vec::new();
    for pat in DATE_PATS {
        let re = Regex::new(pat).expect("date regex compile");
        for m in re.find_iter(text) {
            // 若新匹配区间被已有匹配覆盖（subset），跳过；否则收录。
            let covered = spans
                .iter()
                .any(|(s, e, _)| *s <= m.start() && m.end() <= *e);
            if covered {
                continue;
            }
            spans.push((m.start(), m.end(), m.as_str().to_string()));
        }
    }
    for (s, e, v) in spans {
        out.push(Entity {
            kind: EntityKind::Date,
            value: v,
            byte_start: s,
            byte_end: e,
        });
    }
}

fn extract_case_no(text: &str, out: &mut Vec<Entity>) {
    // (2024)京02民终1234号 / (2023)沪01刑初567号 / (2024)粤民申000号 等
    static CASE_PAT: &str =
        r"\(\s*\d{4}\s*\)\s*[一-鿿]{1,3}\d{0,3}[一-鿿]{1,4}\d{1,6}\s*号";
    let re = Regex::new(CASE_PAT).expect("case_no regex compile");
    for m in re.find_iter(text) {
        push(out, EntityKind::CaseNo, m);
    }
}

fn extract_company(text: &str, out: &mut Vec<Entity>) {
    // 含特定后缀：有限公司 / 股份有限公司 / 有限责任公司 / 研究所 / 事务所 / 学校
    static COMPANY_PAT: &str = r"[一-鿿（）()]{2,15}(?:有限公司|股份有限公司|有限责任公司|研究所|事务所|律师事务所|科技公司|分公司)";
    let re = Regex::new(COMPANY_PAT).expect("company regex compile");
    for m in re.find_iter(text) {
        push(out, EntityKind::Company, m);
    }
}

/// 中文人名启发式：百家姓单字姓 + 1-3 字名（拒绝公司/案号片段）
fn extract_chinese_person(text: &str, out: &mut Vec<Entity>) {
    let common_surnames =
        "李王张刘陈杨赵黄周吴徐孙朱马胡郭林何高梁郑罗宋谢唐韩曹许邓萧冯曾程蔡彭潘袁于董余苏叶吕魏蒋田杜丁沈姜范江";
    let surnames_chars: std::collections::HashSet<char> = common_surnames.chars().collect();
    let chars: Vec<(usize, char)> = text.char_indices().collect();
    let n = chars.len();
    let mut consumed = vec![false; n];

    for i in 0..n {
        if consumed[i] {
            continue;
        }
        let (byte_start, c0) = chars[i];
        if !surnames_chars.contains(&c0) {
            continue;
        }
        let mut name_end_idx = i;
        for j in 1..=3 {
            if i + j >= n {
                break;
            }
            let (_, cj) = chars[i + j];
            if is_chinese_name_char(cj) {
                name_end_idx = i + j;
            } else {
                break;
            }
        }
        if name_end_idx == i {
            continue; // 单字姓没有名 — 跳过
        }
        let byte_end = chars[name_end_idx].0 + chars[name_end_idx].1.len_utf8();
        let value: String = chars[i..=name_end_idx].iter().map(|(_, c)| *c).collect();
        // 拒绝结尾在公司/职务字
        if value.ends_with('总') || value.ends_with('司') || value.ends_with('厂') {
            continue;
        }
        out.push(Entity {
            kind: EntityKind::Person,
            value,
            byte_start,
            byte_end,
        });
        for k in i..=name_end_idx {
            consumed[k] = true;
        }
    }
}

fn is_chinese_name_char(c: char) -> bool {
    let code = c as u32;
    (0x4E00..=0x9FFF).contains(&code) || (0x3400..=0x4DBF).contains(&code)
}

#[cfg(test)]
mod unit_tests {
    use super::*;

    #[test]
    fn money_basic() {
        let v = extract_entities("¥1,000");
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].kind, EntityKind::Money);
    }

    #[test]
    fn date_basic() {
        let v = extract_entities("2024-03-15");
        assert_eq!(v[0].kind, EntityKind::Date);
        assert_eq!(v[0].value, "2024-03-15");
    }

    #[test]
    fn case_no_basic() {
        let v = extract_entities("(2024)京02民终1234号");
        assert_eq!(v[0].kind, EntityKind::CaseNo);
    }

    #[test]
    fn ordering_by_position() {
        let text = "2024-03-15 张三借款 ¥10,000";
        let v = extract_entities(text);
        let kinds: Vec<EntityKind> = v.iter().map(|e| e.kind).collect();
        assert_eq!(kinds, vec![EntityKind::Date, EntityKind::Person, EntityKind::Money]);
    }
}
