//! This is a simple plugin for Perseus that runs the Tailwind CLI at build time.
//! It will automatically download the newest version of the CLI initialize the project to look for
//! class names in Rust files in `src` and HTML files in `static`.
//! Further configuration can be done as usual in `tailwind.config.js`.
//!
//! # Usage
//!
//! Add the plugin to you Perseus App in your Perseus main function.
//!
//! ```
//! # use perseus::PerseusApp;
//! # use perseus::plugins::Plugins;
//! PerseusApp::new()
//!     .plugins(Plugins::new().plugin(
//!         perseus_tailwind::get_tailwind_plugin,
//!         perseus_tailwind::TailwindOptions {
//!             in_file: "src/tailwind.css".into(),
//!             // Don't put this in /static, it will trigger build loops.
//!             // Put this in /dist and use a static alias instead.
//!             out_file: "dist/static/tailwind.css".into(),
//!         },
//!     ))
//!     .static_alias("/static/tailwind.css", "dist/static/tailwind.css")
//! # ;
//! ```
//!
//! If you're already using plugins just add the plugin to your `Plugins` as usual.
//!
//! # Using a custom binary
//!
//! If you for some reason want to use a specific version of the CLI or some other CLI with the same
//! command line interface entirely, just place the binary with its default system-specific name
//! (i.e. `tailwindcss-linux-arm64`) in the project directory.
//!
//! # Stability
//!
//! The plugin is fairly simple and shouldn't break anything since it just executes the Tailwind CLI.
//! The download and installation should work on Linux, MacOS and Windows on all architectures that
//! are supported by Tailwind, but is currently only tested on Windows x64.

#[cfg(not(target_family = "wasm"))]
use perseus::plugins::PluginAction;
use perseus::plugins::{empty_control_actions_registrar, Plugin, PluginEnv};
#[cfg(not(target_family = "wasm"))]
use std::{fs::File, io::Write, path::PathBuf, process::Command};

static PLUGIN_NAME: &str = "tailwind-plugin";

#[cfg(all(target_os = "linux", target_arch = "aarch64"))]
static BINARY_NAME: &str = "tailwindcss-linux-arm64";
#[cfg(all(target_os = "linux", target_arch = "arm"))]
static BINARY_NAME: &str = "tailwindcss-linux-armv7";
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
static BINARY_NAME: &str = "tailwindcss-linux-x64";
#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
static BINARY_NAME: &str = "tailwindcss-macos-arm64";
#[cfg(all(target_os = "macos", target_arch = "x86_64"))]
static BINARY_NAME: &str = "tailwindcss-macos-x64";
#[cfg(all(target_os = "windows", target_arch = "x86_64"))]
static BINARY_NAME: &str = "tailwindcss-windows-x64.exe";

/// Options for the Tailwind CLI
#[derive(Debug)]
pub struct TailwindOptions {
    /// The path to the input CSS file
    pub in_file: String,
    /// The path to the CSS file output by the CLI.\
    /// **DO NOT PUT THIS IN `/static` UNLESS YOU LIKE BUILD LOOPS!**\
    /// Always put it somewhere in `/dist` use static aliases instead.\
    pub out_file: String,
}

/// The plugin constructor
pub fn get_tailwind_plugin<G: perseus::Html>() -> Plugin<G, TailwindOptions> {
    #[allow(unused_mut)]
    Plugin::new(
        PLUGIN_NAME,
        |mut actions| {
            #[cfg(not(target_family = "wasm"))]
            {
                actions
                    .build_actions
                    .before_build
                    .register_plugin(PLUGIN_NAME, |_, data| {
                        if let Some(options) = data.downcast_ref::<TailwindOptions>() {
                            try_run_tailwind(options);
                        } else {
                            unreachable!()
                        }
                    });
                actions
                    .export_actions
                    .before_export
                    .register_plugin(PLUGIN_NAME, |_, data| {
                        if let Some(options) = data.downcast_ref::<TailwindOptions>() {
                            try_run_tailwind(options);
                        } else {
                            unreachable!()
                        }
                    });
            }
            actions
        },
        empty_control_actions_registrar,
        PluginEnv::Server,
    )
}

#[cfg(not(target_family = "wasm"))]
fn try_run_tailwind(options: &TailwindOptions) {
    let cli = PathBuf::from(BINARY_NAME);
    if !cli.exists() {
        install_tailwind_cli();
    }
    if !PathBuf::from("tailwind.config.js").exists() {
        init_tailwind();
    }

    let mut args = vec!["-i", &options.in_file, "-o", &options.out_file];
    if cfg!(not(debug_assertions)) {
        args.push("--minify");
    }

    let output = Command::new(format!("./{BINARY_NAME}"))
        .args(args)
        .output()
        .expect("Failed to run Tailwind CLI");
    let output = String::from_utf8_lossy(&output.stderr);
    // Errors always contain a JSON object. Please start using result codes Tailwind
    // Also, don't write info messages to stderr instead of stdout
    // Also if you're going to print JSON make the whole thing JSON and not some exception stack
    // trace syntax followed by JSON
    if output.contains('}') {
        panic!("{}", output);
    }
}

#[cfg(not(target_family = "wasm"))]
fn install_tailwind_cli() {
    log::info!("Tailwind CLI not found, installing...");
    log::info!("Downloading binary for this platform...");
    let url = format!(
        "https://github.com/tailwindlabs/tailwindcss/releases/latest/download/{BINARY_NAME}"
    );
    let binary = reqwest::blocking::get(url)
        .expect("Failed to download binary. Check it's still available on the tailwind GitHub.")
        .bytes()
        .expect("Failed to read binary content of the tailwind binary download");

    log::info!("Writing to disk as {BINARY_NAME}...");
    let mut file = File::create(BINARY_NAME).expect("Failed to create binary file");
    file.write_all(&binary)
        .expect("Failed to write binary to disk");
    #[cfg(target_family = "unix")]
    {
        println!("Making the binary executable...");
        use std::os::unix::fs::PermissionsExt;
        let mut perms = file
            .metadata()
            .expect("Failed to get metadata for binary to set executable permission")
            .permissions();
        let mode = perms.mode() | 0o550;
        perms.set_mode(mode);
    }
    println!("Done installing Tailwind CLI.");
}

#[cfg(not(target_family = "wasm"))]
fn init_tailwind() {
    log::info!(
        "Initializing Tailwind to search all Rust files in 'src' and all HTML files in 'static'."
    );
    let default_config = include_bytes!("default-config.js");
    let mut config = File::create("tailwind.config.js").expect("Failed to create config file");
    config
        .write_all(default_config)
        .expect("Failed to write default config");
}
