#[cfg(test)]
mod tests {
    use std::path::Path;

    fn is_safe_path(raw: &str) -> bool {
        let p = Path::new(raw);
        if !p.is_absolute() {
            return false;
        }
        // 检查是否包含 .. 组件
        for component in p.components() {
            if component.as_os_str() == ".." {
                return false;
            }
        }
        true
    }

    #[test]
    fn rejects_relative_path() {
        assert!(!is_safe_path("relative/path"));
    }

    #[test]
    fn rejects_path_with_dotdot() {
        assert!(!is_safe_path("/home/user/../../etc/passwd"));
    }

    #[test]
    fn accepts_absolute_home_path() {
        assert!(is_safe_path("/home/user/documents"));
    }
}
