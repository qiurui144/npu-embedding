use crate::error::{Result, VaultError};
use std::path::PathBuf;

/// 给定 HuggingFace repo_id，返回本地缓存目录路径
/// repo_id 中的 '/' 替换为 '_'，避免目录层级问题
pub fn model_cache_dir(repo_id: &str) -> PathBuf {
    crate::platform::models_dir().join(repo_id.replace('/', "_"))
}

/// 确保 model_filename 和 tokenizer_filename 两个文件已缓存在本地
///
/// 若文件不存在则从 HuggingFace Hub 下载（支持 HF_ENDPOINT 环境变量镜像）。
/// 返回 (model_path, tokenizer_path)。
pub fn ensure_models(
    repo_id: &str,
    model_filename: &str,
    tokenizer_filename: &str,
) -> Result<(PathBuf, PathBuf)> {
    let cache_dir = model_cache_dir(repo_id);
    std::fs::create_dir_all(&cache_dir)
        .map_err(|e| VaultError::Crypto(format!("create model dir: {e}")))?;

    // 取文件名末段（model_filename 可能含路径如 "onnx/model_quantized.onnx"）
    let model_basename = model_filename.rsplit('/').next().unwrap_or(model_filename);
    let tokenizer_basename = tokenizer_filename.rsplit('/').next().unwrap_or(tokenizer_filename);

    let model_path = cache_dir.join(model_basename);
    let tokenizer_path = cache_dir.join(tokenizer_basename);

    if model_path.exists() && tokenizer_path.exists() {
        return Ok((model_path, tokenizer_path));
    }

    let api = hf_hub::api::sync::Api::new()
        .map_err(|e| VaultError::Crypto(format!("hf-hub init: {e}")))?;
    let repo = api.model(repo_id.to_string());

    if !model_path.exists() {
        let src = repo.get(model_filename)
            .map_err(|e| VaultError::Crypto(format!("download {model_filename}: {e}")))?;
        std::fs::copy(&src, &model_path)
            .map_err(|e| VaultError::Crypto(format!("copy model file: {e}")))?;
    }

    if !tokenizer_path.exists() {
        let src = repo.get(tokenizer_filename)
            .map_err(|e| VaultError::Crypto(format!("download {tokenizer_filename}: {e}")))?;
        std::fs::copy(&src, &tokenizer_path)
            .map_err(|e| VaultError::Crypto(format!("copy tokenizer file: {e}")))?;
    }

    Ok((model_path, tokenizer_path))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn model_cache_dir_for_repo() {
        let dir = model_cache_dir("Qwen/Qwen3-Embedding-0.6B");
        assert!(dir.to_str().unwrap().contains("Qwen_Qwen3-Embedding-0.6B"));
    }

    #[test]
    fn model_cache_dir_replaces_slash() {
        let dir = model_cache_dir("BAAI/bge-reranker-v2-m3");
        let s = dir.to_str().unwrap();
        assert!(!s.contains("BAAI/bge"), "slash should be replaced");
        assert!(s.contains("BAAI_bge-reranker-v2-m3"));
    }
}
