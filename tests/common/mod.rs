pub enum StreamSource<S: AsRef<str>> {
    Stdin(S),
    File(S),
}

pub fn run_grab<S: AsRef<str>>(args: &[&str], source: StreamSource<S>) -> String {
    #[allow(deprecated)]
    let mut cmd = assert_cmd::Command::cargo_bin("grab").unwrap();

    cmd.args(args);

    match source {
        StreamSource::Stdin(input) => {
            cmd.write_stdin(
                std::fs::read_to_string(input.as_ref()).expect("failed to read input file"),
            );
        }
        StreamSource::File(file) => {
            cmd.arg(file.as_ref());
        }
    }

    let output = cmd.output().expect("failed to execute process");

    format!(
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8(output.stdout).expect("invalid UTF-8 output"),
        String::from_utf8(output.stderr).expect("invalid UTF-8 output")
    )
}
