//! F1 "basic": six files in three directories, five commits by two authors,
//! and no agent signature anywhere.

use std::io;

use crate::{Fixture, Identity, RepoBuilder};

/// The commit id F1 must have on every machine. It was checked against an
/// independent computation of the object hashes, so it depends on nothing
/// but the recipe below.
pub const HEAD: &str = "822e3536cbae5f3404efab40519ba1de1102e6a0";

/// The paths at HEAD, sorted.
pub const FILES: [&str; 6] = [
    "docs/guide.md",
    "docs/notes.md",
    "src/app.py",
    "src/models.py",
    "src/util.py",
    "tests/test_app.py",
];

/// The number of commits reachable from HEAD.
pub const COMMITS: usize = 5;

const ADA: Identity = Identity {
    name: "Ada Example",
    email: "ada@example.com",
};

const GRACE: Identity = Identity {
    name: "Grace Example",
    email: "grace@example.com",
};

/// 2026-01-05 09:00:00 UTC. Each commit is one day after the one before.
const START: i64 = 1_767_603_600;
const DAY: i64 = 86_400;

/// Builds the fixture in a fresh temporary directory.
pub fn build() -> io::Result<Fixture> {
    let repo = RepoBuilder::new()?;

    repo.write(
        "src/app.py",
        "import util\n\n\ndef main():\n    print(util.greet(\"world\"))\n",
    )?;
    repo.write(
        "src/util.py",
        "def greet(name):\n    return \"hello \" + name\n",
    )?;
    repo.commit(ADA, START, "Add the app and a greeting helper\n")?;

    repo.write(
        "tests/test_app.py",
        "import util\n\n\ndef test_greet():\n    assert util.greet(\"a\") == \"hello a\"\n",
    )?;
    repo.write(
        "src/app.py",
        "import util\n\n\ndef main():\n    print(util.greet(\"world\"))\n\n\n\
         if __name__ == \"__main__\":\n    main()\n",
    )?;
    repo.commit(
        GRACE,
        START + DAY,
        "Test the greeting and make the app runnable\n",
    )?;

    repo.write("docs/guide.md", "# Guide\n\nRun `python src/app.py`.\n")?;
    repo.commit(ADA, START + 2 * DAY, "Document how to run the app\n")?;

    repo.write(
        "src/models.py",
        "class Greeting:\n    def __init__(self, name):\n        self.name = name\n",
    )?;
    repo.write(
        "src/util.py",
        "from models import Greeting\n\n\ndef greet(name):\n    \
         return \"hello \" + Greeting(name).name\n",
    )?;
    repo.commit(
        GRACE,
        START + 3 * DAY,
        "Introduce a Greeting model\n\n\
         The helper now builds a Greeting instead of using the bare name.\n",
    )?;

    repo.write(
        "docs/notes.md",
        "# Notes\n\n- The greeting is English only.\n",
    )?;
    repo.write(
        "docs/guide.md",
        "# Guide\n\nRun `python src/app.py`.\n\nRun the tests with `pytest`.\n",
    )?;
    repo.commit(ADA, START + 4 * DAY, "Add notes and mention the tests\n")?;

    repo.finish()
}
