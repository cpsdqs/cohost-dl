#[macro_use]
extern crate log;

use crate::bundled_files::TEMPLATE_CONFIG;
use crate::context::CohostContext;
use crate::data::Database;
use crate::import_cdl1::CohostDl1ImportConfig;
use anyhow::Context;
use clap::{Parser, Subcommand};
use diesel::connection::SimpleConnection;
use diesel::{Connection, SqliteConnection};
use diesel_migrations::{embed_migrations, EmbeddedMigrations, MigrationHarness};
use rustyline::error::ReadlineError;
use rustyline::DefaultEditor;
use serde::Deserialize;
use std::env::{current_dir, current_exe};
use std::path::PathBuf;
use std::str::FromStr;
use std::time::Duration;
use std::{env, fs, process};
use tokio::time::sleep;

mod bundled_files;
mod comment;
mod context;
mod data;
mod dl;
mod feed;
mod import_cdl1;
mod login;
mod merge;
mod post;
mod project;
mod render;
mod res_ref;
mod schema;
mod server;
mod trpc;

const MIGRATIONS: EmbeddedMigrations = embed_migrations!("migrations");

#[derive(Debug, Parser)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// Downloads data as specified in config.toml
    Download,
    /// Starts a local web server to view downloaded data
    Serve,
    /// Generates a new config.toml in the current directory
    GenerateConfig,
    /// Updates an existing config.toml with a new session cookie (interactive)
    Login,
    /// Imports cohost-dl 1 data (interactive)
    ImportCohostDl1,
    /// Imports data from another cohost-dl 2 download
    ///
    /// This will copy posts, comments, and files from the other download into the current download.
    MergeData {
        /// Other database file
        database: String,
        /// Other file data directory
        files: String,
    },
}

#[derive(Debug, Deserialize)]
pub struct Config {
    pub database: String,
    pub cookie: String,
    pub request_timeout_secs: Option<u64>,
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
    pub load_tagged_posts: Vec<String>,
    #[serde(default)]
    pub load_specific_posts: Vec<String>,
    #[serde(default)]
    pub skip_follows: Vec<String>,
    #[serde(default)]
    pub load_new_posts: bool,
    #[serde(default)]
    pub load_comments: bool,
    #[serde(default)]
    pub try_fix_transparent_shares: bool,
    #[serde(default)]
    pub load_post_resources: bool,
    #[serde(default)]
    pub load_project_resources: bool,
    #[serde(default)]
    pub load_comment_resources: bool,
    #[serde(default)]
    pub forget_missing_url_files: bool,
    #[serde(default)]
    pub skip_inaccessible_profiles: bool,
    pub server_port: u16,
}

fn main() {
    // MS Windows apparently has quite a small default stack size, so we can't use the main thread.
    // I am not figuring out MSVC linker arguments for this
    std::thread::Builder::new()
        .stack_size(8 * 1024 * 1024)
        .name("cohost_dl::main".into())
        .spawn(|| {
            let rt = tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .thread_stack_size(8 * 1024 * 1024)
                .build()
                .unwrap();
            rt.block_on(main_impl());
        })
        .unwrap()
        .join()
        .unwrap();
}

async fn main_impl() {
    if env::var("RUST_LOG").is_err() {
        env::set_var("RUST_LOG", "info");
    }
    env_logger::init();

    let args = Cli::parse();
    if let Some(command) = args.command {
        let (config, db) = match init() {
            Ok(init) => init,
            Err(e) => {
                eprintln!("{e:?}");
                process::exit(1);
            }
        };

        match command {
            Commands::Download => dl::download(config, db).await,
            Commands::Serve => server::serve(config, db, || {}).await,
            Commands::GenerateConfig => {
                let path = PathBuf::from("config.toml");
                if path.exists() {
                    println!("Refusing to overwrite existing config.toml!");
                    process::exit(1);
                }
                fs::write(path, TEMPLATE_CONFIG).unwrap();
            }
            Commands::Login => {
                if let Err(e) = interactive_login_with_existing_config().await {
                    eprintln!("{e:?}");
                    process::exit(1);
                }
            }
            Commands::ImportCohostDl1 => {
                if let Err(e) = interactive_import_cdl1_data(config, db).await {
                    eprintln!("{e:?}");
                    process::exit(1);
                }
            }
            Commands::MergeData {
                database: other_db,
                files: other_root_dir,
            } => {
                if let Err(e) = merge::merge(
                    &Database::new(db),
                    &other_db,
                    &PathBuf::from(config.root_dir),
                    &PathBuf::from(other_root_dir),
                )
                .await
                {
                    eprintln!("{e:?}");
                    process::exit(1);
                }
            }
        }
    } else {
        interactive().await;
    }
}

fn init() -> anyhow::Result<(Config, SqliteConnection)> {
    let config = fs::read_to_string("config.toml").context("could not read config.toml")?;
    let config: Config = toml::from_str(&config).context("error reading config")?;

    let mut db =
        SqliteConnection::establish(&config.database).context("could not open database")?;
    db.batch_execute("pragma foreign_keys = on; pragma journal_mode = WAL;")
        .context("could not set up database")?;

    if let Err(e) = db.run_pending_migrations(MIGRATIONS) {
        anyhow::bail!("could not run database migrations: {e}");
    }

    Database::migrate_old_url_files(&mut db)?;
    Database::migrate_posts(&mut db)?;

    Ok((config, db))
}

async fn interactive() {
    // set cwd to binary location in interactive mode because we can probably assume the user
    // launched it by double-clicking the binary, which would have cwd ~ by default.
    let mut bin_dir = current_exe().expect("could not determine current path");
    bin_dir.pop();
    env::set_current_dir(bin_dir).expect("could not set current path");

    if let Err(e) = interactive_impl().await {
        eprintln!("{e:?}");
        process::exit(1);
    }
}

async fn wizard() {
    const WIZARD: [&str; 11] = [
        r"   *                  *          ",
        r"     .+..__         ⠄  *         ",
        r"   +IIIIIIII==+  *           *   ",
        r" |I/  \IIIIIII+-.oOOo+*..        ",
        r" +    //OOOOOOOOO+WWWWW|+  *  ⠄  ",
        r"     //|OOOO=+    --++'     ⣦    ",
        r"  +  /oOO=+ ⣤   ⠛  \.    ⠠⠴⣾⣽⡷⠒⠁ ",
        r" *  /.=+      ⠘⠚⠒⠁  |       ⢕    ",
        r"    .+              /.   *  |    ",
        r"    +              /        |    ",
        r"                 /  *     ='|    ",
    ];

    for line in WIZARD {
        sleep(Duration::from_millis(50)).await;
        println!("{}", line);
    }

    sleep(Duration::from_millis(1000)).await;
}

fn rl_parse<T>(prompt: &str, mut handler: impl FnMut(&str) -> Option<T>) -> anyhow::Result<T> {
    let mut rl = DefaultEditor::new()?;
    loop {
        match rl.readline(prompt) {
            Ok(s) => {
                if let Some(result) = handler(s.trim()) {
                    break Ok(result);
                }
            }
            Err(ReadlineError::Eof) => {
                println!("end of input. goodbye!");
                process::exit(0);
            }
            Err(ReadlineError::Interrupted) => {
                println!("interrupted. goodbye!");
                process::exit(0);
            }
            Err(ReadlineError::WindowResized) => continue,
            Err(e) => Err(e)?,
        }
    }
}

fn interactive_yn(prompt: &str) -> anyhow::Result<bool> {
    rl_parse(&format!("{prompt} [Y/N] "), |yn| {
        match &*yn.to_ascii_lowercase() {
            "y" | "ye" | "yes" => Some(true),
            "n" | "no" => Some(false),
            _ => {
                println!("Enter 'yes' or 'no'.");
                None
            }
        }
    })
}

async fn interactive_impl() -> anyhow::Result<()> {
    let bin_name = current_exe()?
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or("cohost-dl".into());

    println!("-- cohost-dl 2 interactive wizard --");
    println!("(run `{bin_name} help` to see other commands)\n");
    wizard().await;
    println!("A wizard appears before you.");
    println!();

    let config_path = PathBuf::from("config.toml");
    let has_config = config_path.exists();
    if has_config {
        println!("A `config.toml` file is here.");
        interactive_has_config().await
    } else {
        println!("There does not appear to be a `config.toml` file here.");
        println!();
        println!("The wizard is offering to walk you through creating a configuration file.");

        let accept = interactive_yn("Accept?")?;
        if accept {
            interactive_setup().await
        } else {
            println!("You can configure cohost-dl yourself by using the `generate-config` subcommand to create a template file.");
            Ok(())
        }
    }
}

async fn interactive_setup() -> anyhow::Result<()> {
    let cwd = env::current_dir()?;
    let mut config = toml_edit::DocumentMut::from_str(TEMPLATE_CONFIG)?;

    println!("-- cohost-dl 2 interactive setup --");
    println!();
    println!("All of these settings will be saved in a new `config.toml` file,");
    println!("and you can change them later.");
    println!();

    println!("1. Where do you want to put the downloaded post data?");
    println!("   This is text data stored in a database (≈1 GB for 400K posts).");
    println!("   You can enter e.g. 'data' to use a file called 'data.db'");
    println!("   in the current directory.");
    println!();

    let database = loop {
        let path = rl_parse("file path: ", |p| Some(p.to_string()))?;

        if path.is_empty() {
            continue;
        }

        let mut abs_path = current_dir()?.join(path);

        if abs_path.is_dir() {
            println!("This file path cannot be used because it points to a folder.");
            continue;
        }
        if abs_path.file_name().is_none() {
            println!("No file name specified.");
            continue;
        }
        abs_path.set_extension("db");

        let maybe_rel_path = if let Ok(path) = abs_path.strip_prefix(&cwd) {
            PathBuf::from(path)
        } else {
            abs_path.clone()
        };

        let Some(path_str) = maybe_rel_path.to_str() else {
            println!("This file path contains invalid UTF-8. This is not supported, sorry!");
            continue;
        };

        println!(
            "Data will be put in a file at this location:\n{}",
            abs_path.display()
        );

        if abs_path.exists() {
            println!("Warning!! A file already exists here");
        }

        let ok = interactive_yn("Is this ok?")?;
        if ok {
            break path_str.to_string();
        }
    };

    println!();
    println!("2. Where do you want to put downloaded image & audio data?");
    println!("   This could get quite large (≈ 100 GB for 400K posts).");
    println!("   You can enter e.g. 'files' to use a folder called 'files'");
    println!("   in the current directory.");
    println!();

    let out_path = loop {
        let path = rl_parse("folder path: ", |p| Some(p.to_string()))?;

        if path.is_empty() {
            continue;
        }

        let abs_path = current_dir()?.join(path);

        if abs_path.is_file() {
            println!("This folder path cannot be used because it points to a file.");
            continue;
        }

        let maybe_rel_path = if let Ok(path) = abs_path.strip_prefix(&cwd) {
            PathBuf::from(path)
        } else {
            abs_path.clone()
        };

        let Some(path_str) = maybe_rel_path.to_str() else {
            println!("This file path contains invalid UTF-8. This is not supported, sorry!");
            continue;
        };

        if path_str == database {
            println!(
                "This folder path cannot be used because that’s where the database will be stored."
            );
            continue;
        }

        println!(
            "Data will be put in a folder at this location:\n{}",
            abs_path.display()
        );

        if abs_path.exists() {
            println!("Warning!! A folder already exists here");
        }

        let ok = interactive_yn("Is this ok?")?;
        if ok {
            break path_str.to_string();
        }
    };

    println!();
    println!("3. Would you like the wizard to log you into Cohost,");
    println!("   or would you like to provide your own session cookie?");
    println!();

    let cookie = loop {
        let login = interactive_yn("Have the wizard log you in?")?;

        if login {
            match interactive_login().await? {
                Some(result) => break result,
                None => continue,
            }
        } else {
            let cookie = rl_parse("session cookie: ", |i| {
                if i.is_empty() {
                    return Some(String::new());
                }

                let header = i.trim();
                let header_lower = header.to_ascii_lowercase();
                if !header_lower.starts_with("connect.sid=s%3a") {
                    println!("This does not appear to be a valid session cookie.");
                    println!("It should look something like `connect.sid=s%3AB8…<lots of base64>");
                    return None;
                }

                Some(header.to_string())
            })?;

            if !cookie.is_empty() {
                break cookie;
            }
        };
    };

    println!();
    println!("Checking...");

    let db = SqliteConnection::establish(&database).context("opening database")?;
    let ctx = CohostContext::new(
        cookie.clone(),
        Duration::from_secs(60),
        PathBuf::from(&out_path),
        db,
    );
    let login = ctx.login_logged_in().await.context("getting login info")?;
    let projects = ctx
        .projects_list_edited_projects()
        .await
        .context("getting login info")?;

    drop(ctx);

    let current_handle = projects
        .projects
        .iter()
        .find(|p| p.project_id == login.project_id)
        .map(|p| format!("@{}", p.handle))
        .unwrap_or("(error)".into());

    println!("Logged in as {current_handle}");

    if projects.projects.len() > 1 {
        println!("Your account has access to pages other than {current_handle}!");
        println!("cohost-dl currently can’t switch the active page.");
        println!("If you used a browser cookie to log in, you can switch pages in the browser.");
    }

    println!();
    println!("4. What do you want to download?");

    println!();
    println!("- Download all posts from {current_handle}?");
    let load_self = interactive_yn(&format!("Download posts from {current_handle}?"))?;

    println!();
    println!("- Download all of your liked posts?");
    let load_likes = interactive_yn("Download liked posts?")?;

    println!();
    println!("- Download your entire dashboard?");
    println!("  Your dashboard contains every post from everyone you follow.");
    println!("  That’s probably a lot of posts.");
    let load_dashboard = interactive_yn("Download dashboard?")?;

    println!();
    println!("- Download comments on posts?");
    println!("  This isn’t lots of data; it just takes a while.");
    let load_comments = interactive_yn("Download comments?")?;

    println!();
    println!("Saving configuration...");

    config["database"] = toml_edit::value(database);
    config["root_dir"] = toml_edit::value(out_path);
    config["cookie"] = toml_edit::value(cookie);

    if load_self {
        if let Some(profiles) = config["load_profile_posts"].as_array_mut() {
            profiles.clear();
            // strip @
            profiles.push(&current_handle[1..]);
        }
    }
    if load_likes {
        config["load_likes"] = toml_edit::value(true);
    }
    if load_dashboard {
        config["load_dashboard"] = toml_edit::value(true);
    }
    if load_comments {
        config["load_comments"] = toml_edit::value(true);
    }

    if let Some(tagged) = config["load_tagged_posts"].as_array_mut() {
        tagged.clear();
    }

    config["load_post_resources"] = toml_edit::value(true);
    config["load_project_resources"] = toml_edit::value(true);
    config["load_comment_resources"] = toml_edit::value(true);

    fs::write("config.toml", config.to_string()).context("saving configuration")?;

    println!("You can configure additional options in the `config.toml` file,");
    println!("like loading posts from specific pages, tags, etc.");
    println!();

    let start_dl = interactive_yn("Start downloading now?")?;
    if start_dl {
        let (config, db) = init()?;
        dl::download(config, db).await;

        let serve = interactive_yn("Open results in your browser?")?;
        if serve {
            let (config, db) = init()?;
            println!();
            println!("You can press Ctrl + C to quit.");
            serve_and_open(config, db).await;
        }
    } else {
        println!("You can run the program again later to start downloading.");
    }

    Ok(())
}

async fn interactive_login_with_existing_config() -> anyhow::Result<()> {
    let config = fs::read_to_string("config.toml")?;

    let mut config = toml_edit::DocumentMut::from_str(&config)?;

    let Some(cookie) = interactive_login().await? else {
        println!("Leaving configuration as-is. Bye!");
        sleep(Duration::from_secs(2)).await;
        return Ok(());
    };

    config["cookie"] = toml_edit::value(cookie);

    fs::write("config.toml", config.to_string())?;

    println!("Success! You can now restart the program again to do whatever.");
    sleep(Duration::from_secs(5)).await;
    Ok(())
}

async fn interactive_login() -> anyhow::Result<Option<String>> {
    let (cookie, needs_otp) = loop {
        println!("Enter your Cohost login email address, or type 'exit' to go back.");
        let email = rl_parse("email: ", |i| Some(i.to_string()))?;
        let email = email.trim();

        if email == "exit" {
            return Ok(None);
        }

        println!(
            "Enter your Cohost login password. For security reasons, your input is invisible."
        );
        let password = rpassword::prompt_password("password: ")?;

        println!("Logging in...");
        match login::login(email, &password).await {
            Ok(res) => break res,
            Err(e) => {
                println!("Error: {e:?}");
            }
        }
    };

    if needs_otp {
        loop {
            println!("Enter your 2FA code, or type 'exit' to go back.");
            let code = rl_parse("code: ", |i| Some(i.to_string()))?;
            let code = code.trim();

            if code == "exit" {
                return Ok(None);
            }

            match login::login_otp(&cookie, &code).await {
                Ok(()) => break,
                Err(e) => {
                    println!("Error: {e:?}");
                }
            }
        }
    }

    Ok(Some(cookie))
}

impl Config {
    fn print_dl_info(&self) {
        println!("- data will be saved to {}", self.database);
        println!("- files will be saved to {}", self.root_dir);
        println!();
        if self.load_dashboard {
            println!("- will load dashboard (if not already loaded)");
        }
        if self.load_likes {
            println!("- will load liked posts (if not already loaded)");
        }
        match self.load_profile_posts.len() {
            0 => (),
            1 => println!("- will load posts from 1 page (if not already loaded)"),
            n => println!("- will load posts from {n} page (if not already loaded)"),
        }
        match self.load_tagged_posts.len() {
            0 => (),
            1 => println!("- will load posts from 1 tag (if not already loaded)"),
            n => println!("- will load posts from {n} tags (if not already loaded)"),
        }
        match self.load_specific_posts.len() {
            0 => (),
            1 => println!("- will load 1 specific post from URL (if not already loaded)"),
            n => println!("- will load {n} specific posts from URLs (if not already loaded)"),
        }
        if self.load_new_posts {
            println!("- will check every project for new posts");
        }
        if self.load_comments {
            println!("- will load comments for all posts (if not already loaded)");
        }
        if self.try_fix_transparent_shares {
            println!("- will try to fix transparent shares");
        }
        if self.load_post_resources {
            println!("- will load images and audio files used in posts");
        }
        if self.load_project_resources {
            println!("- will load avatars and headers for pages");
        }
        if self.load_comment_resources {
            println!("- will load images used in comments");
        }
    }
}

async fn interactive_has_config() -> anyhow::Result<()> {
    println!();
    println!("The wizard is offering you following services:");
    println!("(1) downloading data according to configuration");
    println!("(2) looking at downloaded data in your web browser");
    println!("---");
    println!("(3) logging in to cohost again (in case login stopped working)");
    println!("(4) importing data from cohost-dl 1");
    println!();
    println!("You can also type 'exit' to leave.");

    loop {
        let choice = rl_parse("> ", |i| match i {
            "1" => Some(1),
            "2" => Some(2),
            "3" => Some(3),
            "4" => Some(4),
            "exit" | "quit" | "leave" | "bye" => {
                println!("Goodbye!");
                process::exit(0)
            }
            _ => {
                println!("Enter 1, 2, or 'exit'");
                None
            }
        })?;

        let (config, db) = match init() {
            Ok(res) => res,
            Err(e) => {
                error!("{e:?}");
                println!("It appears an error occurred.");
                println!("Maybe your configuration file is invalid?");
                continue;
            }
        };

        match choice {
            1 => {
                println!("The wizard hands off to the downloader and leaves.");
                println!();
                config.print_dl_info();

                sleep(Duration::from_millis(500)).await;

                dl::download(config, db).await;

                let serve = interactive_yn("Open results in your browser?")?;
                if serve {
                    let (config, db) = init()?;
                    println!();
                    println!("You can press Ctrl + C to quit.");
                    serve_and_open(config, db).await;
                }
                break Ok(());
            }
            2 => {
                println!("The wizard hands off to your web browser and leaves.");
                println!();
                println!("You can press Ctrl + C to quit.");

                serve_and_open(config, db).await;
                break Ok(());
            }
            3 => {
                drop(config);
                drop(db);
                if let Err(e) = interactive_login_with_existing_config().await {
                    eprintln!("{e:?}");
                }
                break Ok(());
            }
            4 => {
                println!("The wizard will now import your cohost-dl 1 data.");
                println!();

                interactive_import_cdl1_data(config, db).await?;
                break Ok(());
            }
            _ => (),
        }
    }
}

async fn serve_and_open(config: Config, db: SqliteConnection) {
    let port = config.server_port;

    server::serve(config, db, || {
        if let Err(e) = webbrowser::open(&format!("http://localhost:{port}")) {
            eprintln!("could not open web browser: {e}");
        }
    })
    .await;
}

async fn interactive_import_cdl1_data(config: Config, db: SqliteConnection) -> anyhow::Result<()> {
    println!("Where is your cohost-dl 1 data?");
    let from_dir = rl_parse("path to the `out` directory: ", |i| {
        if i.is_empty() {
            return None;
        }
        let path = PathBuf::from(i);
        let Ok(path) = path.canonicalize() else {
            println!("That file path doesn’t exist");
            return None;
        };
        if !path.is_dir() {
            println!("That’s not a directory");
            return None;
        }
        let rc_dir = path.join("rc");
        if !rc_dir.is_dir() {
            println!("That doesn’t appear to be a cohost-dl 1 `out` directory.");
            return None;
        }
        Some(path)
    })?;
    println!("-> {}", from_dir.display());

    println!();
    println!(
        "What do you want to happen if data for a particular post already exists in the database?"
    );
    println!("(1) overwrite existing data");
    println!("(2) add new data only");

    let add_only = rl_parse("> ", |i| match i {
        "1" => Some(false),
        "2" => Some(true),
        _ => {
            println!("Enter 1 or 2");
            None
        }
    })?;

    println!();
    if add_only {
        println!("When adding a post that didn’t already exist, do you want to try reloading it from cohost.org?");
    } else {
        println!("When adding a post, do you want to try reloading it from cohost.org?");
    }
    println!("(This will, of course, take more time)");

    let reload = interactive_yn("reload?")?;

    println!();
    println!("Configuration:");
    println!("- from: {}", from_dir.display());
    if add_only {
        println!("- add new posts only");
    } else {
        println!("- overwrite posts that already exist");
    }
    if reload {
        println!("- when adding a post, try to reload it from cohost.org");
    } else {
        println!("- do not reload from cohost.org");
        println!("  (note: some data will still be downloaded to fill some missing info)");
    }

    println!();
    println!("OK to start?");
    let ok = rl_parse("[yes/exit] ", |i| match i {
        "y" | "ye" | "yes" => Some(true),
        "exit" => Some(false),
        _ => {
            println!("Enter 'yes' or 'exit'");
            None
        }
    })?;

    if !ok {
        return Ok(());
    }

    let import_config = CohostDl1ImportConfig {
        path: from_dir,
        add_only,
        reload,
    };

    dl::import_cdl1(config, db, import_config).await;
    Ok(())
}
