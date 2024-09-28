#[macro_use]
extern crate log;

use anyhow::Context;
use clap::{Parser, Subcommand};
use diesel::connection::SimpleConnection;
use diesel::{Connection, SqliteConnection};
use diesel_migrations::{embed_migrations, EmbeddedMigrations, MigrationHarness};
use serde::Deserialize;
use std::fs;

mod comment;
mod context;
mod data;
mod dl;
mod liked;
mod post;
mod project;
mod res_ref;
mod schema;
mod server;
mod trpc;

const MIGRATIONS: EmbeddedMigrations = embed_migrations!("migrations");

#[derive(Debug, Parser)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    Download,
    Serve,
}

#[derive(Debug, Deserialize)]
pub struct Config {
    pub database: String,
    pub cookie: String,
    pub root_dir: String,
    #[serde(default)]
    pub do_not_fetch_domains: Vec<String>,
    #[serde(default)]
    pub load_dashboard: bool,
    #[serde(default)]
    pub load_likes: bool,
    #[serde(default)]
    pub load_profile_posts: Vec<String>,
    #[serde(default)]
    pub skip_follows: Vec<String>,
    #[serde(default)]
    pub load_comments: bool,
    #[serde(default)]
    pub load_post_resources: bool,
    #[serde(default)]
    pub load_project_resources: bool,
    #[serde(default)]
    pub load_comment_resources: bool,
    pub server_port: u16,
}

#[tokio::main]
async fn main() {
    if std::env::var("RUST_LOG").is_err() {
        std::env::set_var("RUST_LOG", "info");
    }
    env_logger::init();

    let config = fs::read_to_string("config.toml").expect("could not read config.toml");
    let config: Config = toml::from_str(&config)
        .context("error reading config")
        .unwrap();

    let mut db = SqliteConnection::establish(&config.database).unwrap();
    db.batch_execute("pragma foreign_keys = on; pragma journal_mode = WAL;")
        .unwrap();

    db.run_pending_migrations(MIGRATIONS).unwrap();

    let args = Cli::parse();
    match args.command {
        Commands::Download => dl::download(config, db).await,
        Commands::Serve => server::serve(config, db).await,
    }
}
