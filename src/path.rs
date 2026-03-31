/// A fully-qualified Rust path, e.g. ["a", "b", "c", "OldName"]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RustPath(pub Vec<String>);

impl RustPath {
    pub fn parse(s: &str) -> Result<Self, String> {
        if s.is_empty() {
            return Err("path must not be empty".to_string());
        }
        let segments: Vec<String> = s.split("::").map(|seg| seg.to_string()).collect();
        for seg in &segments {
            if seg.is_empty() {
                return Err(format!("invalid path segment in '{s}'"));
            }
        }
        Ok(RustPath(segments))
    }

    pub fn last(&self) -> &str {
        self.0.last().expect("RustPath must not be empty")
    }

    pub fn parent(&self) -> Option<RustPath> {
        if self.0.len() <= 1 {
            None
        } else {
            Some(RustPath(self.0[..self.0.len() - 1].to_vec()))
        }
    }

    pub fn is_prefix_of(&self, other: &RustPath) -> bool {
        other.0.starts_with(&self.0)
    }

    /// If `self` starts with `from` as a prefix, return a new path
    /// where that prefix is replaced by `to`.
    pub fn rewrite_prefix(&self, from: &RustPath, to: &RustPath) -> Option<RustPath> {
        if self.0.starts_with(&from.0) {
            let mut new_segs = to.0.clone();
            new_segs.extend_from_slice(&self.0[from.0.len()..]);
            Some(RustPath(new_segs))
        } else {
            None
        }
    }

    pub fn to_string(&self) -> String {
        self.0.join("::")
    }
}

impl std::fmt::Display for RustPath {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0.join("::"))
    }
}
