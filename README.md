# KubeCTX-Edit

Small software utility to efficiently manage multiple kubeconfig contexts.

Add, Remove and Edit entries in kubeconfig

## Build and Run

```bash
cargo build --release --
./target/release/kubectx-edit
```

## Kubeconfig in none default location

If your kubeconfig in none default location, you can set the `KUBECONFIG` environment variable to the path of your kubeconfig file.

```bash
export KUBECONFIG=/path/to/your/kubeconfig
```

## Editor

You can set the `EDITOR` environment variable to the editor of your choice. (default `vi`)

```bash
export EDITOR=nano
```
