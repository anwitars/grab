mod common;
use common::*;

mod happy {
    use super::*;

    #[test]
    fn test_simple() {
        insta::assert_snapshot!(run_grab(
            &["--mapping", "name,age,email", "--skip", "1", "--json"],
            StreamSource::File("tests/fixtures/simple.csv"),
        ))
    }

    #[test]
    fn test_stdin() {
        insta::assert_snapshot!(run_grab(
            &["--mapping", "name,age,email", "--skip", "1", "--json"],
            StreamSource::Stdin("tests/fixtures/simple.csv"),
        ))
    }

    #[test]
    fn test_empty() {
        insta::assert_snapshot!(run_grab(
            &["--mapping", "column", "--json"],
            StreamSource::File("tests/fixtures/empty.csv"),
        ))
    }

    #[test]
    fn test_ending_with_newline() {
        insta::assert_snapshot!(run_grab(
            &["--mapping", "name,age,email", "--skip", "1", "--json"],
            StreamSource::File("tests/fixtures/simple_newline.csv"),
        ))
    }

    #[test]
    fn test_colspan() {
        insta::assert_snapshot!(run_grab(
            &[
                "--mapping",
                "a:2,first_name,last_name,b:8",
                "--skip",
                "1",
                "--select",
                "first_name,last_name",
                "--json",
            ],
            StreamSource::File("tests/fixtures/many_columns.csv"),
        ))
    }

    #[test]
    fn test_greedy() {
        insta::assert_snapshot!(run_grab(
            &[
                "--mapping",
                "a:2,first_name,last_name,b:g",
                "--skip",
                "1",
                "--select",
                "first_name,last_name",
                "--json",
            ],
            StreamSource::File("tests/fixtures/many_columns.csv"),
        ))
    }

    #[test]
    fn test_placeholder() {
        insta::assert_snapshot!(run_grab(
            &[
                "--mapping",
                "_:2,first_name,last_name,_:g",
                "--skip",
                "1",
                "--json",
            ],
            StreamSource::File("tests/fixtures/many_columns.csv"),
        ))
    }

    #[test]
    fn test_loose() {
        insta::assert_snapshot!(run_grab(
            &["--mapping", "name", "--skip", "1", "--loose"],
            StreamSource::File("tests/fixtures/simple.csv"),
        ))
    }

    #[test]
    fn test_loose_extra_column() {
        insta::assert_snapshot!(run_grab(
            &[
                "--mapping",
                "name,age,email,extra,extra_colspan:2,extra_greedy:g",
                "--skip",
                "1",
                "--loose",
                "--json"
            ],
            StreamSource::File("tests/fixtures/simple.csv"),
        ))
    }
}

mod unhappy {
    use super::*;

    #[test]
    fn missing_columns() {
        insta::assert_snapshot!(run_grab(
            &["--mapping", "name,age,email", "--skip", "1", "--json",],
            StreamSource::File("tests/fixtures/missing_columns.csv")
        ))
    }
}
