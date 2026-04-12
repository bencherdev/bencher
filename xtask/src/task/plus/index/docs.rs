use url::Url;

use crate::parser::TaskIndexDocs;

use super::update::Update;

const BENCHER_DEV: &str = "https://bencher.dev";

// Canonical list lives in `services/console/src/i18n/ui.ts:6-16`.
// English is handled separately (no prefix), so this only holds the 8 prefixed locales.
const NON_EN_LANGS: [&str; 8] = ["de", "es", "fr", "ja", "ko", "pt", "ru", "zh"];

#[derive(Debug)]
pub struct Docs {
    update: Update,
}

impl TryFrom<TaskIndexDocs> for Docs {
    type Error = anyhow::Error;

    fn try_from(docs: TaskIndexDocs) -> Result<Self, Self::Error> {
        let TaskIndexDocs { engine, path } = docs;
        if path.is_empty() {
            anyhow::bail!("Path is empty");
        }
        let url = path
            .into_iter()
            .map(|p| expand_path(&p))
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .flatten()
            .collect();
        Ok(Self {
            update: Update::new(engine.map(Into::into), url),
        })
    }
}

impl Docs {
    pub async fn exec(&self) -> anyhow::Result<()> {
        self.update.exec().await
    }
}

fn expand_path(path: &str) -> anyhow::Result<Vec<Url>> {
    if !path.starts_with('/') {
        anyhow::bail!("Path `{path}` must start with `/`");
    }
    let base = Url::parse(&format!("{BENCHER_DEV}{path}"))?;
    validate_en_path(&base)?;
    // English first (original, unprefixed), then the 8 translated variants.
    let mut urls = vec![base.clone()];
    urls.extend(NON_EN_LANGS.iter().map(|lang| lang_url(&base, lang)));
    Ok(urls)
}

fn validate_en_path(url: &Url) -> anyhow::Result<()> {
    if let Some(first) = url.path_segments().and_then(|mut s| s.next())
        && NON_EN_LANGS.contains(&first)
    {
        anyhow::bail!(
            "Path already has a non-English locale prefix `/{first}/`; pass the English (unprefixed) path"
        );
    }
    Ok(())
}

fn lang_url(base: &Url, lang: &str) -> Url {
    let mut url = base.clone();
    // `base.path()` always starts with `/`, so this yields `/{lang}/...`.
    let new_path = format!("/{lang}{}", base.path());
    url.set_path(&new_path);
    url
}

#[cfg(test)]
mod tests {
    use url::Url;

    use super::{expand_path, lang_url, validate_en_path};

    #[test]
    fn expand_path_constructs_all_urls() {
        let urls = expand_path("/learn/benchmarking/rust/criterion/").unwrap();
        assert_eq!(urls.len(), 9);
        assert_eq!(
            urls.first().map(Url::as_str),
            Some("https://bencher.dev/learn/benchmarking/rust/criterion/"),
        );
        assert_eq!(
            urls.get(1).map(Url::as_str),
            Some("https://bencher.dev/de/learn/benchmarking/rust/criterion/"),
        );
    }

    #[test]
    fn expand_path_rejects_missing_leading_slash() {
        let err = expand_path("learn/benchmarking/").unwrap_err().to_string();
        assert!(err.contains("must start with `/`"), "error was: {err}");
    }

    #[test]
    fn expand_path_rejects_locale_prefix() {
        let err = expand_path("/de/learn/benchmarking/")
            .unwrap_err()
            .to_string();
        assert!(err.contains("/de/"), "error was: {err}");
    }

    #[test]
    fn lang_url_prepends_locale() {
        let base = Url::parse("https://bencher.dev/learn/benchmarking/rust/criterion/").unwrap();
        assert_eq!(
            lang_url(&base, "de").as_str(),
            "https://bencher.dev/de/learn/benchmarking/rust/criterion/",
        );
    }

    #[test]
    fn lang_url_preserves_trailing_slash() {
        let with_slash = Url::parse("https://bencher.dev/learn/").unwrap();
        assert_eq!(
            lang_url(&with_slash, "ja").as_str(),
            "https://bencher.dev/ja/learn/",
        );
        let without_slash = Url::parse("https://bencher.dev/learn").unwrap();
        assert_eq!(
            lang_url(&without_slash, "ja").as_str(),
            "https://bencher.dev/ja/learn",
        );
    }

    #[test]
    fn lang_url_handles_root() {
        let root = Url::parse("https://bencher.dev/").unwrap();
        assert_eq!(lang_url(&root, "fr").as_str(), "https://bencher.dev/fr/");
    }

    #[test]
    fn validate_accepts_en_path() {
        let url = Url::parse("https://bencher.dev/learn/benchmarking/rust/criterion/").unwrap();
        assert!(validate_en_path(&url).is_ok());
    }

    #[test]
    fn validate_accepts_root() {
        let root = Url::parse("https://bencher.dev/").unwrap();
        assert!(validate_en_path(&root).is_ok());
    }

    #[test]
    fn validate_allows_lookalike_path() {
        // `deutsch` starts with `de` but is not an exact segment match.
        let url = Url::parse("https://bencher.dev/deutsch/learn/").unwrap();
        assert!(validate_en_path(&url).is_ok());
    }
}
