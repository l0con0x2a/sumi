# 検証記録

## 環境

2026-11-05、Void Linux/glibc x86_64、Linux 6.18.60_1 で確認しました。
Rust 1.89.0 と 1.99.1 を使用しています。
端末テストは自分で作成した疑似端末だけを使い、OS の IME、エディタ、tmux、ユーザー設定には依存しません。

musl 静的リンク版も、同じ Linux ホスト上で実行しました。
musl 版 Void の実環境でのインストールやパッケージ作成は、まだ確認していません。

## 自動テスト

通常構成は、単体テストと統合テスト、ドキュメントテストを合わせて 70 件です。
端末機能を無効にした構成でも、残りのテストを確認します。

|対象|確認内容|
|---|---|
|エンジン|モード、かな、候補移動、確定、取消し、送り仮名、入力状態の不変条件|
|ローマ字|断片の境界による結果の一貫性、標準 SKK と common の二重 n、外来音と記号|
|辞書|注釈、送りブロック、安全な文字列復号、構文エラー、BOM、優先順位|
|学習|再起動後の候補順、競合、古いスナップショットからの保存、コメント保持、symlink|
|通信|要求と応答の対応、状態取得、リセット、候補指定、登録、バージョン拒否、フレーム超過からの回復|
|CLI|かなフィルタ、複数辞書、JSON 検索、UTF-8 でないパス、単語登録、Broken pipe|
|端末|本文編集、結合濁点と絵文字、候補一覧、貼り付け、小画面、登録、キャンセル|
|終了処理|SIGINT、SIGTERM、SIGHUP、Ctrl-Z と再開、端末設定の復元|

次の構成をローカルで確認しています。

```sh
cargo test --locked
cargo test --no-default-features --locked
cargo +1.89.0 test --locked
cargo test --release --target x86_64-unknown-linux-musl --locked
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo clippy --all-targets --no-default-features -- -D warnings
RUSTDOCFLAGS='-D warnings' cargo doc --no-deps --locked
```

配布アーカイブは、通常ビルドと別のビルド先を指定して検証します。

```sh
CARGO_TARGET_DIR=target/package-check cargo package --locked
```

GitHub Actions にも、最小 Rust 版、stable、端末機能なし、musl、整形と Clippy の設定を追加しました。
GitHub 上での実行は未確認です。

## 実辞書

SKK-JISYO.L を調査用の一時ディレクトリに取得し、EUC-JP から UTF-8 に変換しました。
辞書本体はリポジトリや配布アーカイブに含めていません。

- 取得元: https://raw.githubusercontent.com/skk-dev/dict/master/SKK-JISYO.L
- 元ファイル SHA-256: `c791f578d1b4040fce282db29bc22b2cc7ea46f83e269fab2e0fa779e2967e40`
- UTF-8 版 SHA-256: `d61619eb772fe1ca0f37bedbb0804614db679d546a9d2fbae77a514c0e51a2ca`
- UTF-8 版サイズ: 6,156,797 byte

`skk convert かんじ` で、漢字、幹事、監事、感じなど 12 候補を取得しました。
`check-dict` の結果は次の通りです。

```json
{"entries":147503,"candidates":203549,"okuri_blocks":0,"skipped_entries":28286,"skipped_candidates":3}
```

ASCII の見出し語、数値変換などの対応外エントリは読み飛ばし、対応外の候補式も実行しません。
全エントリの検索内容を別の SKK と照合したわけではありません。

## 遅延

同じ実辞書を使い、`Kanji Space Enter` を 10,000 回、計 70,000 キー処理しました。
下表は 1 回の参考値で、OS のスケジューリングや CPU の動作状態によって変動します。

```sh
cargo build --release --locked
cargo run --release --example latency -- /path/to/SKK-JISYO.L.utf8
cargo run --release --example stdio_latency -- ./target/release/skk /path/to/SKK-JISYO.L.utf8
```

|計測|p50|p95|p99|最大|
|---|---:|---:|---:|---:|
|Engine のキー処理|611 ns|1,603 ns|1,663 ns|87,023 ns|
|stdio の要求と応答|40,235 ns|58,831 ns|73,258 ns|439,176 ns|

辞書の読み込みは約 261 ms、stdio プロセスの起動と辞書読み込みは約 264 ms でした。
辞書統合では、最初の大辞書を再構築せず、ユーザー辞書は該当する見出し語だけに重ねています。

Engine の計測には Update の生成と候補の複製を含めます。
stdio の計測は、1 つの子プロセスとの往復、JSON の生成と解析を含めます。
端末描画、入力装置や端末エミュレータの遅延、ユーザー辞書の同期保存は含めません。

初期 MVP の計測とアダプタの技術検証は、[過去の検証記録](history/mvp-validation.md) に分けています。
