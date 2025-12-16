# Packaged JMeter (pjmeter)

Packaged-JMeter allows developers to have unified JMeter directory structure in their projects, automatic handling of plugins based on projects' configuration and a defined pattern for execution reports. 

[![CI](https://github.com/vduda-zirnitra/pjmeter/actions/workflows/ci.yml/badge.svg)](https://github.com/vduda-zirnitra/pjmeter/actions/workflows/ci.yml)

## Features
- Command-line interface built in Rust
- User specific configuration
- Each project has its own configuration and plugins
- Design with variables configured in config.toml files
- Run on, a selection of or all tests
- Failproof on JMeter's hanging on 'wait on threads' termination

## Installation

### Prebuilt binaries
- Download from Releases: <releases-url>
- Install with : sudo dpkg -i "pjmeter_<version>-<debian_version>_<architecture>.deb"
 
### Runtime prerequisites
:warning: The user running pjmeter must belong to the group `pjmeter`. This is mandatory for plugin usage.

The group is automatically created when installing with the .deb file, the user installing `pjmeter` is automatically added to this group but might need to restart its session to be properly recognized as belonging to the group.

## Quickstart
### Summary
`pjmeter` relies on environment variables to configure each project with JMeter, the designer must first list all the variables that will be needed (eg. url, port).

Then, when designing a test plan, those environment variables should be accessed in JMeter with `${__groovy(System.getenv("ENV_VAR"))}`. Please check how it's done in the sample given when initiating a new project.

### First execution
```bash
# 1) Initialize a new project (the path has to exist on the file system). This should be made once, so that the project is registered / initiated if needed.
pjmeter project add --label my-project --path /absolute/path/to/my/project

# 2) Design with JMeter for a project
pjmeter design --project my-project --environment typicode

# 3) Run all test plans
pjmeter run --project my-project --environment typicode
```

Please note that `pjmeter` also works with an interactive prompt. Simply try:
```bash
pjmeter
```
Then navigate to :
- "Project" -> "Add" : to add a new project by following the prompts
- "Design" : to design on a project selected from the prompted list and on a prompted environment
- "Run" : to run a project's test plans on an environment, all selected by prompts

### JMeter
See [User's Manual](https://jmeter.apache.org/usermanual/index.html) 

## Configuration

### User global pjmeter configuration
Each user has its own `pjmeter` configuration, stored as an SQLite file (see [Data storage (SQLite)](#data-storage-sqlite))

### Project configuration
Each project has its own configuration that comes as a .toml file in `<project_path>/jmeter/config.toml`.\
When initiating a project, a sample config file is written. Edit it to suit your project's needs.

Example `config.toml`:
```toml
[jmeter]
# When running PJMeter in design mode, you can choose to automatically open a default test plan.
# This parameter is optional : comment it not to use it.
default_test_plan = "a_sample_plan.jmx"

# JMeter sometimes fail to realize that all its threads are finished.
# If the .jml file doesn't change for the amount of time given (in seconds), then
# pjmeter kills JMeter, deletes the last line from the .jml file and generates the report.
# This parameter is optional : comment if not to use it. If not used, JMeter might hang indefinitely waiting for its threads to finish.
inactivity_kill_timeout = 60 # in seconds

[plugins]
# Add the plugins your project will use in the following section.
#jpgc-casutg = "3.1.1"
#jpgc-dummy = "0.4"
#jpgc-functions = "2.2"
#jpgc-graphs-basic = "2.0"
#jpgc-graphs-composite = "2.0"

# This environment is used for a_sample_plan.jmx, delete it and use your own env. configuration.
[env.typicode]
PROTOCOL = "https"
TARGET_ENV_URL = "jsonplaceholder.typicode.com"
TARGET_ENV_PORT = "443"

# Add your environments below, as child tables of [env]
# each entry will be an environment variable available in your .jmx files.
#[env.local]
#PROTOCOL = "https"
#TARGET_ENV_URL = "localhost"
#TARGET_ENV_PORT = "8080"
```

## Data storage (SQLite)
- User's data path: `~/.local/share/pjmeter/config.db` - specific to each user on the host machine, stores defined projects ;
- Application's configuration : `/var/opt/pjmeter/config.db` - global to all users, stores JMeter plugins

## Logging
- Logs to stdout and stderr
- JMeter log file can be found in each project

## Exit codes
- 0: Success
- 1: General error
- 2: Invalid CLI usage

## Security notes
- Do not run as root.
- Network access:
  - JMeter's plugins are downloaded from : https://jmeter-plugins.org/files/packages/

## Troubleshooting
- Waiting for user issues

## Roadmap
- [ ] Auto-set the label when adding a project (options: use the directory name or a .toml configuration property)
- [ ] Add an update checker, checking if a new release is available
- [ ] Add `healthcheck` behaviors:
  - check if .toml files are correct for each project
  - check if projects are still where expected from the settings
  - check if projects' `jmeter/` directory structure is still valid
- [ ] Enhanced plugin management
  - Check plugins compatibility
  - Allow the use of caret to get the last version of a plugin (following SemVer)
- [ ] Plan manipulation
  - Semi-automatically update plans to add environment variables when user says plans of a project should be updated. KEY = variable name
  - Initialize a new plan with a default template and all environment variables set
  - Rename an environment variable => should update the environment variable in all plans
  - Check in which plan an environment variable is used and at which step
  - Check for safe delete of an environment variable.
- [ ] Have an available scripts collection: with "Plan manipulation", give a way to add a script step so that a user can use pre-made scripts

---

# Development

## :warning: Critical note :warning:
This project relies on JMeter to work properly, it is downloaded by given scripts (see [Initiate your development environment](#initiate-your-development-environment)).

**The downloaded JMeter must not be altered since it can't be commited and can't be shared by other developers.**

**When packaging the application, the downloaded JMeter is also the one packaged, so no modifications can be made to it.**

## :information_source: Use cargo_db.sh instead of cargo
This project is using sqlx to store configuration in SQLite database files. To compile, sqlx needs an existing database to check types on.

In order to avoid developer actions to get the project running, you should use `cargo_db.sh` script given in this project. This script initiate a database in `target/sqlx/build-check.db`.

## Prerequisites
- Rust <MSRV> via rustup
- Get libssl-dev and pkg-config: ```sudo apt-get install libssl-dev pkg-config```
- Install sqlx-cli crate : ```cargo install sqlx-cli```

## Initiate your development environment
To initiate your development environment, you need to execute:

```bash
./init_dev_environment.sh
```

This will:
- download JMeter in `.jmeter-app/`
- initiate `.sandbox/`

## Build and run
```bash
./cargo_db.sh build
./cargo_db.sh run -- <args>
```

:information_source: the crate `sqlx` requires to have a database file on which it can make controls when building.

## Tests
```bash
# Unit + integration tests
./cargo_db.sh test
```

## Lint/format
```bash
./cargo_db.sh fmt --all
./cargo_db.sh clippy --all-targets -- -D warnings
```

## Database migrations
The developer can find the SQLite database file in `~/.local/share/pjmeter/config.db`.

`pjmeter` creates a new SQLite database for each user, the database migration is automatic when executed.


## Releasing
- Update version (SemVer), changelog
- Tag and push: `git tag vX.Y.Z && git push --tags`
- Build debian file: `./build_debian.sh`
- Create a release on Gitlab with the generated .deb file.
