use std::path::PathBuf;

const APP_DIR: &str = "attune";
const LEGACY_APP_DIR: &str = "npu-vault";

pub fn data_dir() -> PathBuf {
    // 容器/headless 环境中 dirs::data_local_dir() 可能返回 None（无 HOME 变量）；
    // 回退到 $HOME/.local/share 或当前目录，确保不 panic。
    //
    // 迁移规则：老目录 npu-vault/ 若存在且新目录 attune/ 不存在，返回老路径（就地复用，
    // 避免升级丢数据）。新建用户使用 attune/。
    let base = dirs::data_local_dir()
        .or_else(|| std::env::var("HOME").ok().map(|h| PathBuf::from(h).join(".local/share")))
        .unwrap_or_else(|| PathBuf::from("."));
    resolve_app_dir(base)
}

pub fn config_dir() -> PathBuf {
    // 同上，回退到 $HOME/.config 或当前目录
    let base = dirs::config_dir()
        .or_else(|| std::env::var("HOME").ok().map(|h| PathBuf::from(h).join(".config")))
        .unwrap_or_else(|| PathBuf::from("."));
    resolve_app_dir(base)
}

/// 迁移兼容：新老目录名都认。老安装返回老路径、新安装用新名字。
fn resolve_app_dir(base: PathBuf) -> PathBuf {
    let new_path = base.join(APP_DIR);
    let legacy_path = base.join(LEGACY_APP_DIR);
    if !new_path.exists() && legacy_path.exists() {
        legacy_path
    } else {
        new_path
    }
}

pub fn db_path() -> PathBuf {
    data_dir().join("vault.db")
}

pub fn device_secret_path() -> PathBuf {
    config_dir().join("device.key")
}

/// 模型缓存目录：~/.local/share/attune/models/（老路径 npu-vault/ 自动兼容）
pub fn models_dir() -> PathBuf {
    data_dir().join("models")
}

/// 可用的硬件加速后端
#[derive(Debug, Clone, PartialEq)]
pub enum NpuKind {
    IntelNpu,
    IntelIgpu,
    AmdNpu,
    Cuda,
    None,
}

/// 探测本机最优 Execution Provider
///
/// 优先级：NPU_VAULT_EP 环境变量 > CUDA > CPU fallback
pub fn detect_npu() -> NpuKind {
    match std::env::var("NPU_VAULT_EP").as_deref() {
        Ok("openvino") => NpuKind::IntelNpu,
        Ok("directml") => NpuKind::AmdNpu,
        Ok("cuda") => NpuKind::Cuda,
        Ok("cpu") | Ok("none") => NpuKind::None,
        _ => {
            if std::path::Path::new("/dev/nvidia0").exists() {
                NpuKind::Cuda
            } else {
                NpuKind::None
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths_end_with_attune_or_legacy() {
        // 迁移期：新安装使用 attune/，老安装保持 npu-vault/。两者都认。
        let dd = data_dir();
        let cd = config_dir();
        let ends_ok = |p: &PathBuf| p.ends_with(APP_DIR) || p.ends_with(LEGACY_APP_DIR);
        assert!(ends_ok(&dd), "data_dir should end with attune or npu-vault: {:?}", dd);
        assert!(ends_ok(&cd), "config_dir should end with attune or npu-vault: {:?}", cd);
    }

    #[test]
    fn db_path_inside_data_dir() {
        let db = db_path();
        assert!(db.starts_with(data_dir()));
        assert_eq!(db.file_name().unwrap(), "vault.db");
    }

    #[test]
    fn device_secret_inside_config_dir() {
        let ds = device_secret_path();
        assert!(ds.starts_with(config_dir()));
        assert_eq!(ds.file_name().unwrap(), "device.key");
    }

    #[test]
    fn models_dir_inside_data_dir() {
        let md = models_dir();
        assert!(md.starts_with(data_dir()));
        assert!(md.to_str().unwrap().ends_with("models"));
    }

    #[test]
    fn detect_npu_returns_valid_variant() {
        let npu = detect_npu();
        let _ = format!("{:?}", npu);
    }

    #[test]
    fn detect_npu_respects_env_var() {
        std::env::set_var("NPU_VAULT_EP", "cuda");
        assert_eq!(detect_npu(), NpuKind::Cuda);
        std::env::set_var("NPU_VAULT_EP", "cpu");
        assert_eq!(detect_npu(), NpuKind::None);
        std::env::remove_var("NPU_VAULT_EP");
    }
}
