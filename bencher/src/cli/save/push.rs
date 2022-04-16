use std::path::Path;

use git2::{Cred, Direction, Error, RemoteCallbacks, Repository};

pub fn push(key: Option<&str>, repo: &Repository) -> Result<(), Error> {
    // Connect remote.
    let mut remote = repo.find_remote("origin")?;
    remote.connect_auth(
        Direction::Push,
        Some(callbacks(key, "Remote connection".into())),
        None,
    )?;

    // Prepare push options.
    let mut po = git2::PushOptions::new();

    // Prepare callbacks.
    po.remote_callbacks(callbacks(key, "Push commit".into()));

    let url: [&str; 1] = ["refs/heads/master:refs/heads/master"];

    // Push remote.
    remote.push(&url, Some(&mut po))
}

fn callbacks(key: Option<&str>, context: String) -> RemoteCallbacks<'_> {
    let mut callbacks = if let Some(key) = key {
        let mut callbacks = RemoteCallbacks::new();
        callbacks.credentials(|_url, username_from_url, _allowed_types| {
            Cred::ssh_key(username_from_url.unwrap(), None, Path::new(key), None)
        });
        callbacks
    } else {
        RemoteCallbacks::new()
    };
    callbacks.push_update_reference(move |name, status| {
        if let Some(status) = status {
            Err(git2::Error::from_str(&format!(
                "Update reference failed: {status}"
            )))
        } else {
            println!("{context} for refspec {name} succeeded");
            Ok(())
        }
    });
    callbacks
}
