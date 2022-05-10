use dropshot::ApiDescription;

mod get;

pub fn register(api: &mut ApiDescription<()>) -> Result<(), String> {
    api.register(get::api_get_reports)
}
