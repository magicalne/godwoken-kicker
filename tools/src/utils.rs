use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    env,
    ffi::OsStr,
    fs, panic,
    path::{Path, PathBuf},
    process::Command,
    vec,
};
use url::Url;

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
pub struct SystemConfig {
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
                    "https://github.com/nervosnetwork/godwoken.git#v0.6.0-rc4",
                ),
                (
                    "godwoken-polyman",
                    "https://github.com/RetricSu/godwoken-polyman.git#v0.6.0-rc2",
                ),
                (
                    "godwoken-web3",
                    "https://github.com/nervosnetwork/godwoken-web3.git#v0.5.0-rc2",
                ),
                (
                    "godwoken-scripts",
                    "https://github.com/nervosnetwork/godwoken-scripts.git#v0.8.0-rc2",
                ),
                (
                    "godwoken-polyjuice",
                    "https://github.com/nervosnetwork/godwoken-polyjuice.git#v0.8.2-rc1",
                ),
                (
                    "clerkb",
                    "https://github.com/nervosnetwork/clerkb.git#v0.4.0",
                ),
            ]
            .iter()
            .map(|(name, url)| PackageInfo {
                repo_name: name.to_string(),
                repo_url: Url::parse(url).expect(&format!("package {} url parse error", name)),
                build_mode: DEFAULT_BUILD_MODE,
            })
            .collect(),
            images_info: [
                (
                    "docker_prebuild_image",
                    "nervos/godwoken-prebuilds",
                    "v0.6.0-rc2",
                ),
                (
                    "docker_manual_build_image",
                    "retricsu/godwoken-manual-build",
                    "latest",
                ),
                (
                    "docker_js_prebuild_image",
                    "nervos/godwoken-js-prebuilds",
                    "v0.6.0-rc2",
                ),
            ]
            .iter()
            .map(|(id, name, tag)| ImageInfo {
                id: id.to_string(),
                image_name: name.to_string(),
                image_tag: tag.to_string(),
            })
            .collect(),
            system: SystemConfig {
                always_fetch_new_package: false,
                build_godwoken_over_docker: false,
            },
        }
    }
}

pub fn generate_default_config_file(output_path: &Path) {
    let config = Config::default();
    let output_content = toml::to_string_pretty(&config).expect("serde toml to string pretty");
    let res = fs::write(output_path, output_content.as_bytes()).map_err(|err| anyhow!("{}", err));
    log::info!("{:?}", res);
}

pub fn read_config() -> Result<Config> {
    let config_dir: &Path = Path::new("./kicker-config.toml");
    let config: Config = {
        let content = fs::read(config_dir)?;
        toml::from_slice(&content)?
    };
    Ok(config)
}

pub fn build_godwoken(repo_dir: &Path, repo_name: &str) {
    let config = read_config().expect("msg");
    if config.system.build_godwoken_over_docker {
        run_in_dir("cargo", &["build"], &repo_dir.display().to_string())
            .expect("failed to build godwoken on local.");
        return;
    }

    // todo: build via docker
    // run("docker", vec!["", repo_name]).expect("run make");
    panic!("build godwoken via docker not impl yet!");
}

pub fn build_godwoken_scripts(repo_dir: &Path, repo_name: &str) {
    let repo_dir = make_path(repo_dir, vec![repo_name]).display().to_string();
    let target_dir = format!("{}/c", repo_dir);
    println!("{:?} ,,,,, {:?}", repo_dir, target_dir);
    run("make", vec!["-C", &target_dir]).expect("run make");
    run_in_dir(
        "capsule",
        vec!["build", "--release", "--debug-output"],
        &repo_dir,
    )
    .expect("run capsule build");
}

pub fn build_godwoken_polyjuice(repo_dir: &Path, repo_name: &str) {
    let target_dir = make_path(repo_dir, vec![repo_name]).display().to_string();
    run("make", vec!["-C", &target_dir, "all-via-docker"]).expect("run make");
}

pub fn build_clerkb(repo_dir: &Path, repo_name: &str) {
    let target_dir = make_path(repo_dir, vec![repo_name]).display().to_string();
    println!("{:?}", target_dir);
    run("yarn", vec!["--cwd", &target_dir]).expect("run yarn");
    run("make", vec!["-C", &target_dir, "all-via-docker"]).expect("run make");
}

pub fn build_node_module_by_copy(repo_dir: &Path, repo_name: &str) {
    let target_dir = make_path(repo_dir, vec![repo_name]).display().to_string();
    if let Err(_err) = run("yarn", vec!["--cwd", &target_dir, "check", "--verify-tree"]) {
        log::info!("yarn check --verify-tree failed, start to copy node_module_from docker..");
        copy_node_module_from_docker(repo_name).expect("copy node_module_failed");
    }
}

pub fn copy_node_module_from_docker(repo_name: &str) -> Result<()> {
    let config = read_config()?;
    // todo: hard-code index to get image is not ideal, change the data structure here
    let image = format!(
        "{}:{}",
        config.images_info[2].image_name, config.images_info[2].image_tag
    );
    let current_dir = env::current_dir().expect("current dir");
    let cmd = format!(
        "docker run --rm -v {}/packages/{}:/app {} /bin/bash -c \"cp -r ./{}/node_modules ./app/\"",
        &current_dir.display().to_string(), repo_name, image, repo_name
    );
    run_one_line_cmd(cmd.as_str())
}

pub fn collect_scripts_to_target(
    repo_dir: &Path,
    target_dir: &Path,
    scripts_info: &HashMap<String, ScriptsInfo>,
) {
    scripts_info.iter().for_each(|(_, v)| {
        let target_path = v.target_script_path(target_dir);
        let source_path = v.source_script_path(repo_dir);
        fs::create_dir_all(&target_path.parent().expect("get dir")).expect("create scripts dir");
        log::debug!("copy {:?} to {:?}", source_path, target_path);
        fs::copy(source_path, target_path).expect("copy script");
    });
}

#[derive(Clone, Serialize, Deserialize, PartialEq, Eq, Debug)]
pub struct ScriptsInfo {
    #[serde(default)]
    source: PathBuf,

    #[serde(default)]
    always_success: bool,
}

impl ScriptsInfo {
    fn source_script_path(&self, repo_dir: &Path) -> PathBuf {
        make_path(repo_dir, vec![self.source.as_path()])
    }

    fn target_script_path(&self, target_root_dir: &Path) -> PathBuf {
        let script_name = self.source.file_name().expect("get script name");
        let repo_name = self
            .source
            .components()
            .next()
            .expect("get repo name")
            .as_os_str();
        make_path(target_root_dir, vec![repo_name, script_name])
    }
}

pub fn provide_godwoken_scripts() {
    let scripts: HashMap<String, ScriptsInfo> = [
        (
            "always_success",
            "godwoken-scripts/build/release/always-success",
        ),
        (
            "custodian_lock",
            "godwoken-scripts/build/release/custodian-lock",
        ),
        (
            "deposit_lock",
            "godwoken-scripts/build/release/deposit-lock",
        ),
        (
            "withdrawal_lock",
            "godwoken-scripts/build/release/withdrawal-lock",
        ),
        (
            "challenge_lock",
            "godwoken-scripts/build/release/challenge-lock",
        ),
        ("stake_lock", "godwoken-scripts/build/release/stake-lock"),
        (
            "tron_account_lock",
            "godwoken-scripts/build/release/tron-account-lock",
        ),
        (
            "state_validator",
            "godwoken-scripts/build/release/state-validator",
        ),
        (
            "eth_account_lock",
            "godwoken-scripts/build/release/eth-account-lock",
        ),
        (
            "l2_sudt_generator",
            "godwoken-scripts/c/build/sudt-generator",
        ),
        (
            "l2_sudt_validator",
            "godwoken-scripts/c/build/sudt-validator",
        ),
        (
            "meta_contract_generator",
            "godwoken-scripts/c/build/meta-contract-generator",
        ),
        (
            "meta_contract_validator",
            "godwoken-scripts/c/build/meta-contract-validator",
        ),
    ]
    .iter()
    .map(|(k, v)| {
        (
            k.to_string(),
            ScriptsInfo {
                source: PathBuf::from(v),
                always_success: false,
            },
        )
    })
    .collect();

    let repo_dir = Path::new("./packages/godwoken");
    let target_dir = Path::new("./workspace/scripts/release");
    collect_scripts_to_target(repo_dir, target_dir, &scripts);

    // collect to backend as well
    let backend_scripts: HashMap<String, ScriptsInfo> = [
        (
            "l2_sudt_generator",
            "godwoken-scripts/c/build/sudt-generator",
        ),
        (
            "l2_sudt_validator",
            "godwoken-scripts/c/build/sudt-validator",
        ),
        (
            "meta_contract_generator",
            "godwoken-scripts/c/build/meta-contract-generator",
        ),
        (
            "meta_contract_validator",
            "godwoken-scripts/c/build/meta-contract-validator",
        ),
    ]
    .iter()
    .map(|(k, v)| {
        (
            k.to_string(),
            ScriptsInfo {
                source: PathBuf::from(v),
                always_success: false,
            },
        )
    })
    .collect();
    collect_scripts_to_target(
        repo_dir,
        Path::new("./workspace/deploy/backend"),
        &backend_scripts,
    );
}

pub fn provide_polyjuice_scripts() {
    let scripts: HashMap<String, ScriptsInfo> = [
        ("polyjuice_generator", "godwoken-polyjuice/build/generator"),
        ("polyjuice_validator", "godwoken-polyjuice/build/validator"),
    ]
    .iter()
    .map(|(k, v)| {
        (
            k.to_string(),
            ScriptsInfo {
                source: PathBuf::from(v),
                always_success: false,
            },
        )
    })
    .collect();

    let repo_dir = Path::new("./packages/godwoken-polyjuice");
    let target_dir = Path::new("./workspace/scripts/release");
    collect_scripts_to_target(repo_dir, target_dir, &scripts);

    // collect to backend as well
    collect_scripts_to_target(
        repo_dir,
        Path::new("./workspace/deploy/polyjuice-backend"),
        &scripts,
    );
}

pub fn provide_clerkb_scripts() {
    let scripts: HashMap<String, ScriptsInfo> = [
        ("poa", "clerkb/build/debug/poa"),
        ("state", "clerkb/build/debug/state"),
    ]
    .iter()
    .map(|(k, v)| {
        (
            k.to_string(),
            ScriptsInfo {
                source: PathBuf::from(v),
                always_success: false,
            },
        )
    })
    .collect();

    let repo_dir = Path::new("./packages/clerkb");
    let target_dir = Path::new("./workspace/scripts/release");
    collect_scripts_to_target(repo_dir, target_dir, &scripts);
}

pub fn provide_godwoken_bin(){
    fs::copy("packages/godwoken/target/debug/godwoken", "workspace/bin/godwoken").expect("copy godwoken bin");
    fs::copy("packages/godwoken/target/debug/gw-tools", "workspace/bin/godwoken").expect("copy gw-tools bin"); 
}

pub fn provide_basic_files(){
    fs::copy("./config/private_key", "./workspace/deploy/private_key").expect("copy private key script");
    run_one_line_cmd("sh ./docker/layer2/init_config_json.sh").expect("init godwoken config json");
}

pub fn create_workspace_folders(){
    let target_paths = vec!["workspace/bin", "workspace/deploy/backend", "workspace/deploy/polyjuice-backend", "workspace/scripts/release"];
    let target_paths: Vec<&Path> = target_paths.iter().map(|p|{
        Path::new(p)
    }).collect();
    for p in target_paths {
        fs::create_dir_all(&p.parent().expect("get dir")).expect("create scripts dir");
    }
}

pub fn prepare_workspace() {
    create_workspace_folders();
    // copy block-producer private key and some init config file
    provide_basic_files();
    provide_godwoken_bin();
    provide_godwoken_scripts();
    provide_polyjuice_scripts();
    provide_clerkb_scripts();
}

pub fn build_package() -> Result<()> {
    let config = read_config()?;
    for p in config.packages_info {
        let dir_str = "./packages/".to_owned() + p.repo_name.as_str();
        let package_repo_dir = Path::new(&dir_str);
        let packages_root_dir = Path::new("./packages/");
        println!("{:?}", p.repo_name);
        if p.build_mode {
            match p.repo_name.as_str() {
                "godwoken" => build_godwoken(package_repo_dir, p.repo_name.as_str()),
                "godwoken-scripts" => build_godwoken_scripts(packages_root_dir, p.repo_name.as_str()),
                "godwoken-polyjuice" => {
                    build_godwoken_polyjuice(packages_root_dir, p.repo_name.as_str())
                }
                "godwoken-polyman" => {
                    build_node_module_by_copy(package_repo_dir, p.repo_name.as_str())
                }
                "godwoken-web3" => {
                    build_node_module_by_copy(package_repo_dir, p.repo_name.as_str())
                }
                "clerkb" => build_clerkb(packages_root_dir, p.repo_name.as_str()),
                _ => (),
            }
        }
    }
    Ok(())
}

pub fn prepare_package() -> Result<()> {
    log::info!("ready to prepare packages: ...");
    let repo_dir: &Path = Path::new("packages/");
    let config = read_config()?;
    log::info!("{:?}", config);
    for p in config.packages_info {
        if p.build_mode {
            run_pull_code(p.repo_url, true, repo_dir, &p.repo_name);
        }
    }
    Ok(())
}

pub fn run_pull_code(mut repo_url: Url, is_recursive: bool, repo_dir: &Path, repo_name: &str) {
    let commit = repo_url
        .fragment()
        .expect("invalid branch, tag, or commit")
        .to_owned();
    repo_url.set_fragment(None);
    let target_dir = make_path(repo_dir, vec![repo_name]);
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

pub fn run_one_line_cmd(arg: &str) -> Result<()> {
    let bin = "bash";
    log::debug!("[Execute]: {} {:?}", bin, arg);
    let status = Command::new(bin.to_owned())
        .env("RUST_BACKTRACE", "full")
        .arg("-c")
        .arg(arg)
        .status()
        .expect("failed to run docker command");
    if !status.success() {
        println!("{:?}", status);
        Err(anyhow::anyhow!(
            "Exited with status code: {:?}",
            status.code()
        ))
    } else {
        Ok(())
    }
}

pub fn run<I, S>(bin: &str, args: I) -> Result<()>
where
    I: IntoIterator<Item = S> + std::fmt::Debug,
    S: AsRef<OsStr>,
{
    log::debug!("[Execute]: {} {:?}", bin, args);
    println!("{:?}, {:?}", args, bin);
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
            "command error {:?}",
            std::str::from_utf8(&service_status.stderr)
        );
        return false;
    }
}
