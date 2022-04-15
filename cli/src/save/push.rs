use std::path::Path;

use git2::{Cred, Direction, Error, RemoteCallbacks, Repository};

pub fn push(url: &str, key: Option<&str>, repo: &Repository) -> Result<(), Error> {
    // Prepare connect callbacks.
    let mut callbacks = if let Some(key) = key {
        let mut callbacks = RemoteCallbacks::new();
        callbacks.credentials(|_url, username_from_url, _allowed_types| {
            Cred::ssh_key(username_from_url.unwrap(), None, Path::new(key), None)
        });
        callbacks
    } else {
        RemoteCallbacks::new()
    };
    callbacks.push_update_reference(|name, status| {
        println!("{name} {status:?}");
        Ok(())
    });

    // Connect remote.
    let mut remote = repo.find_remote("origin")?;
    remote.connect_auth(Direction::Push, Some(callbacks), None)?;

    // Prepare push options.
    let mut po = git2::PushOptions::new();

    // Prepare callbacks.
    let mut callbacks = if let Some(key) = key {
        let mut callbacks = RemoteCallbacks::new();
        callbacks.credentials(|_url, username_from_url, _allowed_types| {
            Cred::ssh_key(username_from_url.unwrap(), None, Path::new(key), None)
        });
        callbacks
    } else {
        RemoteCallbacks::new()
    };
    callbacks.push_update_reference(|name, status| {
        println!("{name} {status:?}");
        Ok(())
    });
    po.remote_callbacks(callbacks);

    let url: [&str; 1] = ["refs/heads/master:refs/heads/master"];

    // Push remote.
    remote.push(&url, Some(&mut po))
}
