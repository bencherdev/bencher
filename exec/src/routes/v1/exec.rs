use tide::{Request, Response, StatusCode};

pub async fn exec(mut _req: Request<()>) -> tide::Result {
    let mut resp = Response::new(StatusCode::Ok);
    resp.set_body("{ result: \"TBD\" }");
    Ok(resp)
}
