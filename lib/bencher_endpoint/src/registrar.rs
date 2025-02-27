pub trait Registrar {
    fn register(
        api_description: &mut dropshot::ApiDescription<bencher_schema::ApiContext>,
        http_options: bool,
        #[cfg(feature = "plus")] is_bencher_cloud: bool,
    ) -> Result<(), dropshot::ApiDescriptionRegisterError>;
}
