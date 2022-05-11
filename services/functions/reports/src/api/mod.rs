use dropshot::ApiDescription;

mod get;
mod put;

pub fn register(api: &mut ApiDescription<()>) -> Result<(), String> {
    api.register(get::api_get_reports)?;
    api.register(put::api_put_reports)?;
    Ok(())
}
