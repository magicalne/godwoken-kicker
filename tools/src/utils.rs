use std::{env, ffi::OsStr, fs, path::{Path, PathBuf}, process::Command, vec};
use url::Url;
use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Config {
    pub packages_info: Vec<PackageInfo>,
    pub images_info: Vec<ImageInfo>,
    pub system: SystemConfig,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PackageInfo {
    repo_name: String,
    repo_url: Url,
    build_mode: bool,
}
#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct ImageInfo {
    id: String,
    image_name: String,
    image_tag: String,
}
#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct  SystemConfig {
    always_fetch_new_package: bool,
    build_godwoken_over_docker: bool,
}

impl Default for Config {
    fn default() -> Self { 
        const DEFAULT_BUILD_MODE: bool = false;
        Config {
        packages_info: [
            (
                "godwoken",
                "https://github.com/nervosnetwork/godwoken.git#v0.6.0-rc4"
            ),
            (
                "godwoken-polyman",
                "https://github.com/RetricSu/godwoken-polyman.git#v0.6.0-rc2"
            ),
            (
                "godwoken-web3",
                "https://github.com/nervosnetwork/godwoken-web3.git#v0.5.0-rc2"
            ),
            (
                "godwoken-scripts",
                "https://github.com/nervosnetwork/godwoken-scripts.git#v0.8.0-rc2"
            ),
            (
                "godwoken-polyjuice",
                "https://github.com/nervosnetwork/godwoken-polyjuice.git#v0.8.2-rc1"
            ),
            (
                "clerkb",
                "https://github.com/nervosnetwork/clerkb.git#v0.4.0"
            )
        ].iter().map(|(name, url)| {
            PackageInfo {
                repo_name: name.to_string(),
                repo_url: Url::parse(url)
                    .expect(&format!("package {} url parse error", name)),
                build_mode: DEFAULT_BUILD_MODE, 
            }
        })
        .collect(),
        images_info: [
            (
                "docker_prebuild_image",
                "nervos/godwoken-prebuilds",
                "v0.6.0-rc2"
            ),
            (
                "docker_manual_build_image",
                "retricsu/godwoken-manual-build",
                "latest"
            ), 
            (
                "docker_js_prebuild_image",
                "nervos/godwoken-js-prebuilds",
                "v0.6.0-rc2"
            )
        ].iter().map(|(id, name, tag)| {
            ImageInfo {
                id: id.to_string(),
                image_name: name.to_string(),
                image_tag: tag.to_string(), 
            }
        })
        .collect(),
        system: SystemConfig {
            always_fetch_new_package: false,
            build_godwoken_over_docker: false,
        },
    } }
}

pub fn generate_deafult_config_file(output_path: &Path) {
    let config = Config::default();
    let output_content = toml::to_string_pretty(&config).expect("serde toml to string pretty");
    let res = fs::write(output_path, output_content.as_bytes()).map_err(|err| anyhow!("{}", err));
    log::info!("{:?}", res);
}

pub fn prepare_package() -> Result<()> {
    log::info!("ready to prepare packages: ...");
    let repo_dir: &Path = Path::new("packages/");
    let config_dir: &Path = Path::new("./kicker-config.toml");
    let config: Config = {
        let content = fs::read(config_dir)?;
        toml::from_slice(&content)?
    };
    log::info!("{:?}", config);
    for p in config.packages_info {
        if p.build_mode {
            run_pull_code(p.repo_url, true, repo_dir, &p.repo_name);
        }
    }
    Ok(())
}

pub fn run_pull_code(mut repo_url: Url, is_recursive: bool, repos_dir: &Path, repo_name: &str) {
    let commit = repo_url
        .fragment()
        .expect("invalid branch, tag, or commit")
        .to_owned();
    repo_url.set_fragment(None);
    let target_dir = make_path(repos_dir, vec![repo_name]);
    if target_dir.exists() {
        if run_git_checkout(&target_dir.display().to_string(), &commit).is_ok() {
            println!("checkout: {:?}", commit);
            return;
        }
        log::info!("Run git checkout failed, the repo will re-init...");
        fs::remove_dir_all(&target_dir).expect("clean repo dir");
    }
    fs::create_dir_all(&target_dir).expect("create repo dir");
    run_git_clone(repo_url, is_recursive, &target_dir.display().to_string())
        .expect("run git clone");
    run_git_checkout(&target_dir.display().to_string(), &commit).expect("run git checkout");
}

pub fn run_git_clone(repo_url: Url, is_recursive: bool, path: &str) -> Result<()> {
    let mut args = vec!["clone", repo_url.as_str(), path];
    if is_recursive {
        args.push("--recursive");
    }
    run("git", args)
}

pub fn run_git_checkout(repo_dir: &str, commit: &str) -> Result<()> {
    run("git", vec!["-C", repo_dir, "fetch"])?;
    run("git", vec!["-C", repo_dir, "checkout", commit])?;
    run(
        "git",
        vec!["-C", &repo_dir, "submodule", "update", "--recursive"],
    )
}

pub fn make_path<P: AsRef<Path>>(parent_dir_path: &Path, paths: Vec<P>) -> PathBuf {
    let mut target = PathBuf::from(parent_dir_path);
    for p in paths {
        target.push(p);
    }
    target
}

pub fn run_in_dir<I, S>(bin: &str, args: I, target_dir: &str) -> Result<()>
where
    I: IntoIterator<Item = S> + std::fmt::Debug,
    S: AsRef<OsStr>,
{
    let working_dir = env::current_dir().expect("get working dir");
    env::set_current_dir(&target_dir).expect("set target dir");
    let result = run(bin, args);
    env::set_current_dir(&working_dir).expect("set working dir");
    result
}

pub fn run<I, S>(bin: &str, args: I) -> Result<()>
where
    I: IntoIterator<Item = S> + std::fmt::Debug,
    S: AsRef<OsStr>,
{
    log::debug!("[Execute]: {} {:?}", bin, args);
    let status = Command::new(bin.to_owned())
        .env("RUST_BACKTRACE", "full")
        .args(args)
        .status()
        .expect("run command");
    if !status.success() {
        Err(anyhow::anyhow!(
            "Exited with status code: {:?}",
            status.code()
        ))
    } else {
        Ok(())
    }
}

pub fn run_cmd<I, S>(args: I) -> Result<String, String>
where
    I: IntoIterator<Item = S> + std::fmt::Debug,
    S: AsRef<OsStr>,
{
    let bin = "ckb-cli";
    log::debug!("[Execute]: {} {:?}", bin, args);
    let init_output = Command::new(bin.to_owned())
        .env("RUST_BACKTRACE", "full")
        .args(args)
        .output()
        .expect("Run command failed");

    if !init_output.status.success() {
        Err(format!(
            "{}",
            String::from_utf8_lossy(init_output.stderr.as_slice())
        ))
    } else {
        let stdout = String::from_utf8_lossy(init_output.stdout.as_slice()).to_string();
        log::debug!("stdout: {}", stdout);
        Ok(stdout)
    }
}

pub fn check_service_status(name: String) -> bool {
    let mut check_status = Command::new("bash");
    check_status
        .arg("-c")
        .arg(format!("docker-compose ps {}", name));

    let service_status = check_status
        .output()
        .expect("docker-compose ps service command failed");

    if service_status.status.success() {
        let status = match std::str::from_utf8(&service_status.stdout) {
            Ok(v) => v,
            Err(_e) => "unknown",
        };

        print!("service status: {:?}", status);

        if status.contains("   Up   ") {
            return true;
        } else {
            return false;
        }
    } else {
        println!(
            "comand error {:?}",
            std::str::from_utf8(&service_status.stderr)
        );
        return false;
    }
}
