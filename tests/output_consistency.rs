use std::ffi::OsString;

use toms_maid::{Config, Opt};

#[test]
fn ensure_output_consistency() {
    let root_path = std::env::current_dir().expect("can get root project path");
    let files_path = root_path.join("tests/output_consistency");
    let test_file = files_path.join("_test.toml");
    let extension: OsString = "toml".into();
    let files = std::fs::read_dir(&files_path).expect("to read dir content");

    let config = Config::default();

    for file in files {
        let file = file.expect("can get file info");

        println!("{}", file.path().to_string_lossy());

        if file.path().extension() != Some(&extension) {
            continue;
        }

        std::fs::copy(file.path(), &test_file).expect("copy to work");

        let opt = Opt {
            files: vec![test_file.clone()],
            folder: vec![],
            check: false,
            silent: true,
        };

        toms_maid::run(opt, config.clone()).expect("to run without errors");

        // We now check that the result matches the expectations
        let expected_path = file.path().with_extension("toml.out");
        let output = std::fs::read(&test_file).expect("to read test file");
        let expected = std::fs::read(expected_path).expect("to read expected file");

        std::fs::remove_file(&test_file).expect("to be able to delete test file");

        assert_eq!(output, expected, "formatter output should match expected");
    }
}
