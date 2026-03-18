"""SQLite FTS5 全文搜索 + jieba 中文分词辅助"""

import logging

import jieba

logger = logging.getLogger(__name__)

# 初始化 jieba（静默模式）
jieba.setLogLevel(logging.WARNING)


def tokenize_for_search(text: str) -> str:
    """使用 jieba 分词，将文本转换为空格分隔的搜索词

    FTS5 simple tokenizer 按空格分词，所以需要预处理。
    """
    words = jieba.cut_for_search(text)
    return " ".join(w.strip() for w in words if w.strip())


def build_fts_query(query: str) -> str:
    """将用户查询转换为 FTS5 查询语法

    对每个分词结果加 * 前缀匹配，用 OR 连接。
    """
    words = jieba.cut_for_search(query)
    terms = [w.strip() for w in words if len(w.strip()) > 1]
    if not terms:
        return query
    # 使用 OR 连接，每个词加引号避免特殊字符问题
    return " OR ".join(f'"{t}"' for t in terms)
