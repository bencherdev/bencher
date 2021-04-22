use tide::{Request, Response, StatusCode};

pub async fn ping(mut _req: Request<()>) -> tide::Result {
    let mut resp = Response::new(StatusCode::Ok);
    resp.set_body("PONG");
    Ok(resp)
}
