# KubeLift CLI

KubeLift CLI is a cross-platform tool to quickly spin up a self-hosted Kubernetes appliance in your Azure subscription for testing purposes. Once created, the tool will retrieve the administrative kubeconfig for the appliance and change context to it.

[![asciicast](https://asciinema.org/a/557588.svg)](https://asciinema.org/a/557588)

KubeLift uses the [Microsoft CBL-Mariner 2.0 Linux](https://microsoft.github.io/CBL-Mariner/) distribution.

## Usage

```bash
Usage: kubelift [OPTIONS] [COMMAND]

Commands:
  init      Initializes the KubeLift configuration file
  up        Creates and starts an instance of KubeLift
  down      Stops and deletes an instance of KubeLift
  clean     Cleans up instance-related data and kubelift.yml in current directory
  switch    Switches your local Kubernetes configuration to point to the current appliance
  help      Print this message or the help of the given subcommand(s)

Options:
  -d, --debug              Send additional debug information to <stdout>
  -c, --cloud <CLOUD>      Future support for different cloud types. Default is Azure
  -h, --help               Print help (see a summary with '-h')
  -V, --version             Print version
```

## Documentation

See https://kubelift.io for official documentation.

## Technology

KubeLift CLI is built on a foundation of Rust.

### Dependencies/commands executed by this tool:

- Azure CLI - `az`
- KubeCtl - `kubectl`
- SSH client - `ssh`
ß
### Recommended IDE Setup

- [VS Code](https://code.visualstudio.com/)
- [rust-analyzer](https://marketplace.visualstudio.com/items?itemName=rust-lang.rust-analyzer)

## Getting Involved

Contributions or forks of this project are welcomed. If you choose to fork the project, please ensure you avoid the use of the name `KubeLift`, or `kubelift`.

## License

Copyright 2023, The KubeLift Contributors

```
Licensed under the Apache License, Version 2.0 (the "License");
you may not use this file except in compliance with the License.
You may obtain a copy of the License at

    http://www.apache.org/licenses/LICENSE-2.0

Unless required by applicable law or agreed to in writing, software
distributed under the License is distributed on an "AS IS" BASIS,
WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
See the License for the specific language governing permissions and
limitations under the License.
```
