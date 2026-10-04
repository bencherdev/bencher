use std::ops::Not;

#[derive(Debug, Clone, Copy)]
pub enum ArchiveAction {
    Archive,
    Unarchive,
}

impl From<ArchiveAction> for bool {
    fn from(action: ArchiveAction) -> Self {
        match action {
            ArchiveAction::Archive => true,
            ArchiveAction::Unarchive => false,
        }
    }
}

impl Not for ArchiveAction {
    type Output = Self;

    fn not(self) -> Self::Output {
        match self {
            Self::Archive => Self::Unarchive,
            Self::Unarchive => Self::Archive,
        }
    }
}

impl AsRef<str> for ArchiveAction {
    fn as_ref(&self) -> &str {
        match self {
            Self::Archive => "archived",
            Self::Unarchive => "unarchived",
        }
    }
}

impl ArchiveAction {
    pub fn is_archived(self) -> bool {
        !bool::from(self)
    }
}
