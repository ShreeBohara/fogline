# Security

## Reporting a problem

Please report security problems privately through GitHub: open the repository's **Security** tab and choose **Report a vulnerability**. If that option is not shown, open an issue that says only that you have a security report, with no details, and the maintainer will arrange a private channel.

Fogline is pre-alpha and has no supported releases yet. Reports are still welcome, and fixes land on `main`.

## What the tool promises

These are design rules, and a break in any of them is a security bug:

- **It does not modify what it scans.** A scan reads a repository through git and writes nothing inside it.
- **It makes no network requests.** The engine has no HTTP client, and CI fails if one enters its dependencies.
- **It writes nothing unless asked.** Results stay in memory unless you pass an output folder.

Later phases add a local web server and optional language-model features. Their rules are written into the phase specs before the code exists.
