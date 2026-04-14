// npu-vault/crates/vault-core/src/chat.rs

use crate::crypto::Key32;
use crate::error::Result;
use crate::index::FulltextIndex;
use crate::llm::{ChatMessage, LlmProvider};
use crate::search::{allocate_budget, SearchResult, INJECTION_BUDGET};
use crate::store::Store;
use crate::vectors::VectorIndex;
use std::sync::{Arc, Mutex};

/// RAG 对话引擎
pub struct ChatEngine {
    llm: Arc<dyn LlmProvider>,
    store: Arc<Mutex<Store>>,
    fulltext: Arc<Mutex<Option<FulltextIndex>>>,
    vectors: Arc<Mutex<Option<VectorIndex>>>,
    embedding: Arc<Mutex<Option<Arc<dyn crate::embed::EmbeddingProvider>>>>,
    reranker: Arc<Mutex<Option<Arc<dyn crate::infer::RerankProvider>>>>,
}

/// 对话响应
#[derive(Debug, Clone, serde::Serialize)]
pub struct ChatResponse {
    pub content: String,
    pub citations: Vec<Citation>,
    pub knowledge_count: usize,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct Citation {
    pub item_id: String,
    pub title: String,
    pub relevance: f32,
}

impl ChatEngine {
    pub fn new(
        llm: Arc<dyn LlmProvider>,
        store: Arc<Mutex<Store>>,
        fulltext: Arc<Mutex<Option<FulltextIndex>>>,
        vectors: Arc<Mutex<Option<VectorIndex>>>,
        embedding: Arc<Mutex<Option<Arc<dyn crate::embed::EmbeddingProvider>>>>,
        reranker: Arc<Mutex<Option<Arc<dyn crate::infer::RerankProvider>>>>,
    ) -> Self {
        Self { llm, store, fulltext, vectors, embedding, reranker }
    }

    /// RAG 对话：搜索知识库 -> 构建 prompt -> 调用 LLM -> 返回带引用的回答
    pub fn chat(
        &self,
        user_message: &str,
        history: &[ChatMessage],
        dek: &Key32,
    ) -> Result<ChatResponse> {
        // 1. 搜索知识库
        let knowledge = self.search_for_context(user_message, dek, 5)?;

        // 2. 构建 system prompt
        let system = build_rag_system_prompt(&knowledge);

        // 3. 组装完整消息列表
        let mut messages = Vec::new();
        messages.push(ChatMessage::system(&system));
        messages.extend_from_slice(history);
        messages.push(ChatMessage::user(user_message));

        // 4. 调用 LLM
        let response = self.llm.chat_with_history(&messages)?;

        // 5. 提取引用
        let citations: Vec<Citation> = knowledge.iter().map(|k| Citation {
            item_id: k.item_id.clone(),
            title: k.title.clone(),
            relevance: k.score,
        }).collect();

        let knowledge_count = knowledge.len();

        // 6. 自动保存对话到知识库
        self.auto_save_conversation(user_message, &response, dek)?;

        Ok(ChatResponse { content: response, citations, knowledge_count })
    }

    fn search_for_context(&self, query: &str, dek: &Key32, top_k: usize) -> Result<Vec<SearchResult>> {
        let ft_guard = self.fulltext.lock().unwrap();
        let vec_guard = self.vectors.lock().unwrap();
        let emb_guard = self.embedding.lock().unwrap();
        let reranker_guard = self.reranker.lock().unwrap();
        let store_guard = self.store.lock().unwrap();

        let ctx = crate::search::SearchContext {
            fulltext: ft_guard.as_ref(),
            vectors: vec_guard.as_ref(),
            embedding: emb_guard.clone(),
            reranker: reranker_guard.clone(),
            store: &store_guard,
            dek,
        };
        let params = crate::search::SearchParams::with_defaults(top_k);
        let mut results = crate::search::search_with_context(&ctx, query, &params)?;
        allocate_budget(&mut results, INJECTION_BUDGET);
        Ok(results)
    }

    fn auto_save_conversation(&self, user_msg: &str, assistant_msg: &str, dek: &Key32) -> Result<()> {
        let content = format!("用户: {}\n\n助手: {}", user_msg, assistant_msg);
        let title = user_msg.chars().take(50).collect::<String>();
        let store = self.store.lock().unwrap();
        let _ = store.insert_item(dek, &title, &content, None, "ai_chat", None, None);
        Ok(())
    }
}

fn build_rag_system_prompt(knowledge: &[SearchResult]) -> String {
    if knowledge.is_empty() {
        return "你是用户的个人知识助手。知识库中暂无与此问题相关的文档。请正常回答。".into();
    }

    let mut prompt = String::from(
        "你是用户的个人知识助手。以下是从用户本地知识库中检索到的相关文档。\n\
         请基于这些知识回答用户的问题。如果引用了某个文档，请标注 [文档标题]。\n\
         如果知识库中没有相关信息，正常回答即可，不要编造引用。\n\n"
    );

    prompt.push_str("=== 知识库相关文档 ===\n\n");
    for (i, item) in knowledge.iter().enumerate() {
        let content = item.inject_content.as_deref().unwrap_or(&item.content);
        prompt.push_str(&format!(
            "[{}] 《{}》(来源: {}, 相关度: {:.0}%)\n{}\n\n",
            i + 1, item.title, item.source_type,
            item.score * 100.0,
            content
        ));
    }
    prompt.push_str("=== 知识库结束 ===\n");
    prompt
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crypto;
    use crate::llm::MockLlmProvider;

    #[test]
    fn build_rag_prompt_empty_knowledge() {
        let prompt = build_rag_system_prompt(&[]);
        assert!(prompt.contains("暂无"));
    }

    #[test]
    fn build_rag_prompt_with_knowledge() {
        let results = vec![SearchResult {
            item_id: "id1".into(),
            score: 0.85,
            title: "合同A".into(),
            content: "合同内容...".into(),
            source_type: "file".into(),
            inject_content: Some("合同内容...".into()),
        }];
        let prompt = build_rag_system_prompt(&results);
        assert!(prompt.contains("合同A"));
        assert!(prompt.contains("85%"));
        assert!(prompt.contains("知识库"));
    }

    #[test]
    fn citation_serializable() {
        let c = Citation { item_id: "a".into(), title: "T".into(), relevance: 0.9 };
        let json = serde_json::to_string(&c).unwrap();
        assert!(json.contains("relevance"));
    }

    #[test]
    fn chat_engine_with_empty_indices() {
        // ChatEngine with no fulltext/vector indices should still work
        let mock = Arc::new(MockLlmProvider::new("test"));
        mock.push_response("LLM回答".into());

        let store = Arc::new(Mutex::new(Store::open_memory().unwrap()));
        let fulltext: Arc<Mutex<Option<FulltextIndex>>> = Arc::new(Mutex::new(None));
        let vectors: Arc<Mutex<Option<VectorIndex>>> = Arc::new(Mutex::new(None));
        let embedding: Arc<Mutex<Option<Arc<dyn crate::embed::EmbeddingProvider>>>> =
            Arc::new(Mutex::new(None));

        let reranker: Arc<Mutex<Option<Arc<dyn crate::infer::RerankProvider>>>> =
            Arc::new(Mutex::new(None));
        let engine = ChatEngine::new(mock, store, fulltext, vectors, embedding, reranker);
        let dek = crypto::Key32::generate();
        let resp = engine.chat("你好", &[], &dek).unwrap();

        assert_eq!(resp.content, "LLM回答");
        assert_eq!(resp.knowledge_count, 0);
        assert!(resp.citations.is_empty());
    }
}
