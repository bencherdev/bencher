use dropshot::ApiDescription;

mod put;

pub fn register(api: &mut ApiDescription<()>) -> Result<(), String> {
    api.register(put::api_put_migrate)?;
    Ok(())
}
