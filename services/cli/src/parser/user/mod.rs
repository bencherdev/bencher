use bencher_json::{Email, ResourceId, Slug, UserName};
use clap::{Parser, Subcommand, ValueEnum};

use crate::parser::{CliBackend, CliPagination};

pub mod token;

#[derive(Subcommand, Debug)]
pub enum CliUser {
    /// List tokens
    #[clap(alias = "ls")]
    List(CliUserList),
    /// View a user
    #[clap(alias = "get")]
    View(CliUserView),
    // Update a user
    #[clap(alias = "edit")]
    Update(CliUserUpdate),
}

#[derive(Parser, Debug)]
pub struct CliUserList {
    /// User name
    #[clap(long)]
    pub name: Option<UserName>,

    /// User search string
    #[clap(long)]
    pub search: Option<String>,

    #[clap(flatten)]
    pub pagination: CliPagination<CliUsersSort>,

    #[clap(flatten)]
    pub backend: CliBackend,
}

#[derive(ValueEnum, Debug, Clone)]
#[clap(rename_all = "snake_case")]
pub enum CliUsersSort {
    /// Name of the user
    Name,
}

#[derive(Parser, Debug)]
pub struct CliUserView {
    /// User slug or UUID
    pub user: ResourceId,

    #[clap(flatten)]
    pub backend: CliBackend,
}

#[derive(Parser, Debug)]
pub struct CliUserUpdate {
    /// User slug or UUID
    pub user: ResourceId,

    /// User name
    #[clap(long)]
    pub name: Option<UserName>,
    /// User slug
    #[clap(long)]
    pub slug: Option<Slug>,
    /// User email
    #[clap(long)]
    pub email: Option<Email>,
    /// User is admin
    #[clap(long)]
    pub admin: Option<bool>,
    /// User is locked
    #[clap(long)]
    pub locked: Option<bool>,

    #[clap(flatten)]
    pub backend: CliBackend,
}
