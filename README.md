# KubeCTX-Edit

Small software utility to efficiently manage multiple kubeconfig contexts.

Add, Remove and Edit entries in kubeconfig

- [KubeCTX-Edit](#kubectx-edit)
  - [Installation](#installation)
    - [Homebrew](#homebrew)
    - [Download and Run Binary](#download-and-run-binary)
    - [Build from source](#build-from-source)
  - [Build and Run](#build-and-run)
  - [Kubeconfig in none default location](#kubeconfig-in-none-default-location)
  - [Editor](#editor)

## Installation

### Homebrew

```bash
brew install stenstromen/tap/kubectx-edit
```

### Download and Run Binary

- For **MacOS** and **Linux**: Checkout and download the latest binary from [Releases page](https://github.com/Stenstromen/kubectx-edit/releases/latest/)
- For **Windows**: Build the binary yourself.

### Build from source

## Build and Run

```bash
cargo build --release --
./target/release/kubectx-edit
```

## Kubeconfig in none default location

If your kubeconfig is in a non-default location, you can set the `KUBECONFIG` environment variable to the path of your kubeconfig file.

```bash
export KUBECONFIG=/path/to/your/kubeconfig
```

## Editor

You can set the `EDITOR` environment variable to the editor of your choice. (default `vi`)

```bash
export EDITOR=nano
```
