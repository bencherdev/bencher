use git2::{Cred, Error, RemoteCallbacks};
use std::env;
use std::path::Path;

pub fn clone() {
    // Prepare callbacks.
    let mut callbacks = RemoteCallbacks::new();
    callbacks.credentials(|_url, username_from_url, _allowed_types| {
        Cred::ssh_key(
            username_from_url.unwrap(),
            None,
            Path::new(&format!("{}/.ssh/id_ed25519", env::var("HOME").unwrap())),
            None,
        )
    });

    // Prepare fetch options.
    let mut fo = git2::FetchOptions::new();
    fo.remote_callbacks(callbacks);

    // Prepare builder.
    let mut builder = git2::build::RepoBuilder::new();
    builder.fetch_options(fo);

    // Clone the project.
    builder
        .clone(
            "git@github.com:epompeii/rollback.git",
            Path::new("/tmp/rollback"),
        )
        .unwrap();
}
