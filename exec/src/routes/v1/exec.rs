use std::env;
use std::fs;
use std::path::Path;
use std::process::Command;
use std::str;

use serde::{Deserialize, Serialize};
use tide::{Error, Request, Response, StatusCode};

#[derive(Debug, Clone, Deserialize, Serialize)]
struct Code(String);

#[derive(Debug, Clone, Deserialize, Serialize)]
struct Rustc {
    stdout: String,
    stderr: String,
}

pub async fn exec(mut req: Request<()>) -> tide::Result {
    let Code(code) = req.body_json().await?;
    println!("{:#?}", code);

    let work_dir = env::var("DATA_PATH").unwrap_or_else(|_| match Command::new("pwd").output() {
        Ok(cmd) => match str::from_utf8(&cmd.stdout) {
            Ok(stdout) => match Path::new(stdout.to_owned().trim()).join("data").to_str() {
                Some(path) => path.to_owned(),
                None => return stdout.to_owned(),
            },
            Err(err) => return err.to_string(),
        },
        Err(err) => return err.to_string(),
    });

    let main = Path::new(&work_dir).join("src").join("lib.rs");
    match fs::write(main, code) {
        Ok(_) => {}
        Err(err) => return Err(Error::from(err)),
    }

    println!("{:#?}", "CMD");
    let cmd = match Command::new("cargo")
        .arg("test")
        .current_dir(work_dir)
        .output()
    {
        Ok(cmd) => cmd,
        Err(err) => return Err(Error::from(err)),
    };

    let rustc = Rustc {
        stdout: match str::from_utf8(&cmd.stdout) {
            Ok(stdout) => stdout.to_owned(),
            Err(err) => return Err(Error::from(err)),
        },
        stderr: match str::from_utf8(&cmd.stderr) {
            Ok(stderr) => stderr.to_owned(),
            Err(err) => return Err(Error::from(err)),
        },
    };
    let rustc = match serde_json::to_string(&rustc) {
        Ok(json) => json,
        Err(err) => return Err(Error::from(err)),
    };

    let mut resp = Response::new(StatusCode::Ok);
    resp.set_body(rustc);
    Ok(resp)
}
