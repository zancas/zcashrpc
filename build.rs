fn main() {
    std::process::Command::new("git")
        .arg("clone")
        .arg("https://github.com/zingolabs/zcashrpc-api.git")
        .arg(format!(
            "{}/zcashrpc-api",
            &std::env::var("OUT_DIR").unwrap()
        ))
        .output()
        .expect("clone fail");
}
