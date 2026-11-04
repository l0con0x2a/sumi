# 検証記録

このページは初期 MVP の実測記録です。
Kakoune と tmux の実装、および `tests/adapters.sh` は、その後 `../sumi-adapters` リポジトリへ分離しました。
現在の本体のテストはエンジン、かな変換、辞書、stdio 通信、CLI を対象とします。

2026-11-05 / Void Linux glibc x86_64 / Rust 1.99.1 / Kakoune 2026.05.21 / tmux 3.8。

## 自動検証

- `cargo test --locked`: エンジン・辞書・プロトコル9件、実バイナリCLI2件。
- `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`。
- `sh tests/adapters.sh`: 独立tmuxサーバー内の実Kakouneで
  `Kanji Space Enter kana Escape → 漢字かな`、2選択への `ka → か` 挿入。
  実PTYでTUIの漢字確定・キャンセル、指定ペインIDへのbuffer転送も確認。
- `sh -n adapters/skk-tmux tests/adapters.sh`。

統合テストはユーザーのtmuxサーバー・Kakoune設定を利用しない。
サーバー名と一時ディレクトリを分け、終了時に破棄する。
popupそのものの自動回帰テストは未実装。

## 手動の実端末検証

別のtmuxサーバーへPTYクライアントを接続し、popupを開いた。
`Kanji Space Enter Ctrl-D` で元のcatペインへ「漢字」だけを転送し、
二度目のpopupで `ka Escape` を入力して追加転送がないことを確認した。
設定ファイルのprefix + Jでpopupが開くことも確認した。

## 実辞書

skk-dev/dictのSKK-JISYO.Lを調査用の `/tmp` に取得し、iconvでEUC-JPからUTF-8へ変換した。
ファイルはリポジトリに含めていない。

- 取得元: https://raw.githubusercontent.com/skk-dev/dict/master/SKK-JISYO.L
- 元ファイルSHA-256: `c791f578d1b4040fce282db29bc22b2cc7ea46f83e269fab2e0fa779e2967e40`
- UTF-8版SHA-256: `d61619eb772fe1ca0f37bedbb0804614db679d546a9d2fbae77a514c0e51a2ca`
- UTF-8版: 6,156,797byte。
- `skk convert かんじ --dict ...`: 漢字、幹事、監事、感じ等12候補を取得。

## 遅延

```sh
cargo run --release --example latency -- /tmp/sumi-research/SKK-JISYO.L.utf8
```

`Kanji Space Enter` を10,000回、計70,000キー処理した1回の参考値。

|項目|実測|
|---|---:|
|辞書読み込み|184.94ms|
|キー処理p50|421ns|
|p95|1,473ns|
|p99|1,503ns|
|最大|33,053ns|

エンジン内のキー処理だけをInstantで測定し、Updateの生成・候補複製を含む。
OSスケジューリングによる変動はあり、保証値ではない。
stdio IPC、Kakoune通知、ディスクへの返信書き込み、描画遅延は含まない。
Kakouneの応答遅延は今後、端末のキー到着から表示まで別途測定する必要がある。
