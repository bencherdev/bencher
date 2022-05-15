use std::sync::Mutex;

use diesel::pg::PgConnection;
use dropshot::ApiDescription;
use util::Registrar;

const API_NAME: &str = "Bencher";
const API_VERSION: &str = "0.1.0";

fn main() -> Result<(), String> {
    let db_connection = util::db::get_db_connection().map_err(|e| e.to_string())?;

    let mut api = ApiDescription::new();
    register(&mut api)?;

    api.openapi(API_NAME, API_VERSION)
        .write(&mut std::io::stdout())
        .map_err(|e| e.to_string())?;

    Ok(())
}

fn register(api: &mut ApiDescription<Mutex<PgConnection>>) -> Result<(), String> {
    api.register(fn_dba::api::put::api_put_dba_migrate)?;
    api.register(fn_reports::api::put::api_put_reports)?;
    api.register(fn_reports::api::get::api_get_metrics)?;
    Ok(())
}
