# 開発への参加

本体では、SKK エンジン、辞書、共通の CLI と stdio 通信を扱います。
特定のアプリケーションに必要な連携は、本体の外で公開インターフェースを使って実装します。

Rust 1.89 以降でビルドできます。
変更後は、対象の挙動に対応したテストを追加し、次を確認してください。

```sh
cargo test --locked
cargo test --no-default-features --locked
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo clippy --all-targets --no-default-features -- -D warnings
cargo doc --no-deps
CARGO_TARGET_DIR=target/package-check cargo package --locked
```

端末のテストは、自分で作成した疑似端末だけを操作します。
OS の IME、エディタ、tmux、ユーザーの設定ファイルは使いません。
対応する Linux 環境では、静的リンク版も確認できます。

```sh
rustup target add x86_64-unknown-linux-musl
cargo test --release --target x86_64-unknown-linux-musl --locked
```

SKK の挙動を変更するときは、読み、未確定ローマ字、確定文字列、モードをそれぞれ確認します。
先行実装と異なる挙動は、比較した条件と制約を記載してください。
辞書のテストデータには、独自に作成した短いデータを使います。
外部の辞書を追加する場合は、再配布条件を確認する必要があります。

コミットは変更の目的ごとに分け、コミットメッセージは日本語で記述します。
新しい依存を追加する場合は、必要な理由と、端末機能を無効にした場合の影響を説明してください。
stdio 通信を変更した場合は、`docs/protocol.md` も更新します。
