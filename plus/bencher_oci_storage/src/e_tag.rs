//! `ETag`s as S3 gives them: the MD5 of an object put whole or of one part.

use bytes::Bytes;
use md5::{Digest as _, Md5};

/// The hex MD5 of bytes held in chunks.
pub(crate) fn md5_hex<'a, I>(chunks: I) -> String
where
    I: IntoIterator<Item = &'a Bytes>,
{
    let mut md5 = Md5::new();
    for chunk in chunks {
        md5.update(chunk);
    }
    hex::encode(md5.finalize())
}

/// An `ETag` without its quotes, the form in which two `ETag`s compare.
pub(crate) fn unquoted(e_tag: &str) -> &str {
    e_tag.trim_matches('"')
}
