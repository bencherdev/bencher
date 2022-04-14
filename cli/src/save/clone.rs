use git2::{Cred, Error, RemoteCallbacks, Repository};
use std::path::Path;

pub fn clone(url: &str, key: Option<&str>) -> Result<Repository, Error> {
    // Prepare fetch options.
    let mut fo = git2::FetchOptions::new();

    if let Some(key) = key {
        // Prepare callbacks.
        let mut callbacks = RemoteCallbacks::new();
        callbacks.credentials(|_url, username_from_url, _allowed_types| {
            Cred::ssh_key(username_from_url.unwrap(), None, Path::new(key), None)
        });
        fo.remote_callbacks(callbacks);
    }

    // Prepare builder.
    let mut builder = git2::build::RepoBuilder::new();
    builder.fetch_options(fo);

    // Clone the project.
    builder.clone(url, Path::new("/tmp/rollback"))
}
