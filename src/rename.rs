use crate::path::RustPath;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RenameKind {
    RenameOnly,
    MoveOnly,
    MoveAndRename,
}

#[derive(Debug, Clone)]
pub struct Rename {
    pub from: RustPath,
    pub to: RustPath,
    pub kind: RenameKind,
}

impl Rename {
    pub fn new_rename(from: RustPath, to: RustPath) -> Self {
        let kind = RenameKind::RenameOnly;
        Rename { from, to, kind }
    }

    pub fn new_move(from: RustPath, to: RustPath) -> Self {
        let kind = if from.last() != to.last() {
            RenameKind::MoveAndRename
        } else {
            RenameKind::MoveOnly
        };
        Rename { from, to, kind }
    }

    pub fn name_changed(&self) -> bool {
        matches!(self.kind, RenameKind::RenameOnly | RenameKind::MoveAndRename)
    }

    pub fn old_name(&self) -> &str {
        self.from.last()
    }

    pub fn new_name(&self) -> &str {
        self.to.last()
    }
}
