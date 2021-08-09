use std::path::Path;

pub mod utils;

fn main() {
	//utils::generate_deafult_config_file(Path::new("./kicker-config.toml"));
    let res = utils::prepare_package();
	println!("{:?}", res);
    log::info!("{:?}", res);
}
