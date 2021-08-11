use std::path::Path;

pub mod utils;

fn main() {
    //utils::generate_default_config_file(Path::new("./kicker-config.toml"));
    let res = utils::prepare_package();
    println!("prepare_package: {:?}", res);
    log::info!("{:?}", res);

    let res = utils::build_package();
    println!("build package: {:?}", res);

    let res = utils::prepare_workspace();
    println!("prepare workspace: {:?}", res);
}
