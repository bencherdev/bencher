use derive_more::Display;
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

use crate::ValidError;

/// The first line of a commit's message.
#[typeshare::typeshare]
#[derive(Debug, Display, Clone, Eq, PartialEq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(try_from = "String")]
#[cfg_attr(feature = "db", derive(diesel::FromSqlRow, diesel::AsExpression))]
#[cfg_attr(feature = "db", diesel(sql_type = diesel::sql_types::Text))]
pub struct CommitTitle(String);

#[cfg(feature = "db")]
crate::typed_string!(CommitTitle);

impl CommitTitle {
    /// The most characters a title keeps.
    pub const MAX_CHARS: usize = 256;

    /// The message's first line, trimmed and cut to [`Self::MAX_CHARS`] characters. None for a
    /// message whose first line is blank.
    pub fn from_message(message: &str) -> Option<Self> {
        // A bare carriage return ends a line too, so no title holds one.
        let line = message.split(['\n', '\r']).next()?.trim();
        let title = line.chars().take(Self::MAX_CHARS).collect::<String>();
        // A cut can end on whitespace.
        let title = title.trim_end();
        (!title.is_empty()).then(|| Self(title.to_owned()))
    }
}

impl TryFrom<String> for CommitTitle {
    type Error = ValidError;

    fn try_from(title: String) -> Result<Self, Self::Error> {
        if is_valid_commit_title(&title) {
            Ok(Self(title))
        } else {
            Err(ValidError::CommitTitle(title))
        }
    }
}

impl FromStr for CommitTitle {
    type Err = ValidError;

    fn from_str(title: &str) -> Result<Self, Self::Err> {
        Self::try_from(title.to_owned())
    }
}

impl AsRef<str> for CommitTitle {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl From<CommitTitle> for String {
    fn from(title: CommitTitle) -> Self {
        title.0
    }
}

fn is_valid_commit_title(title: &str) -> bool {
    crate::is_valid_non_empty(title)
        && !title.contains(['\n', '\r'])
        && title.chars().count() <= CommitTitle::MAX_CHARS
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::{CommitTitle, is_valid_commit_title};

    // Kills taking any line but the first, ending a line only at a line feed, keeping its
    // surrounding whitespace, cutting by bytes or past 256 characters, keeping whitespace that a
    // cut ends on, and making a title that would not read back from the database.
    #[test]
    fn the_title_is_the_first_line_trimmed_and_cut() {
        let long = "é".repeat(300);
        let cut_on_space = format!("{} tail", "a".repeat(255));
        for (message, title) in [
            (
                "Fix the thing\n\nLonger body\n",
                Some("Fix the thing".to_owned()),
            ),
            (
                "  Fix the thing  \r\nBody",
                Some("Fix the thing".to_owned()),
            ),
            ("Fix the thing\rBody", Some("Fix the thing".to_owned())),
            ("\nSubject after a blank line", None),
            (long.as_str(), Some("é".repeat(256))),
            (cut_on_space.as_str(), Some("a".repeat(255))),
            ("", None),
            (" \n\t\n", None),
        ] {
            let parsed = CommitTitle::from_message(message);
            if let Some(parsed) = &parsed {
                assert!(is_valid_commit_title(parsed.as_ref()), "{parsed:?}");
            }
            assert_eq!(parsed.map(String::from), title, "{message:?}");
        }
    }

    // Kills accepting an empty, padded, multi-line, or overlong title from the database or a
    // response.
    #[test]
    fn a_title_is_one_trimmed_line_of_at_most_256_characters() {
        for (title, valid) in [
            ("Fix the thing".to_owned(), true),
            ("é".repeat(256), true),
            ("é".repeat(257), false),
            (String::new(), false),
            (" Fix".to_owned(), false),
            ("Fix\nthe thing".to_owned(), false),
            ("Fix\rthe thing".to_owned(), false),
        ] {
            assert_eq!(is_valid_commit_title(&title), valid, "{title:?}");
        }
    }
}
