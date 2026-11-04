# sumi / skk

エディタ・OS IMEに依存しない、小さなSKK入力エンジンのMVP。
Rustライブラリ `sumi_skk` と、単一バイナリ `skk` を提供する。
Kakouneアダプタは実験段階。普段の編集環境へ導入する前に以下の制約を確認すること。

## ビルド・インストール

Rust stable（開発・検証: 1.99.1、edition 2024）、Cリンカを用意する。
Void LinuxではRust/Cのビルド環境を用意した上で以下を実行する。
この作業環境ではRustを `/tmp/sumi-cargo` と `/tmp/sumi-rustup` に隔離して導入した。
通常の利用者は自身のRust環境で実行すればよい。

```sh
cargo build --release --locked
cargo install --path . --locked
skk convert かんじ
# 漢字
# 感じ
```

通常の入出力はUTF-8。エンジンの依存はstdのみ。JSON通信に `serde` と `serde_json` を使用する。
端末UIは `/dev/tty` と `stty` を使用し、開始・終了時にだけsttyを起動する。
アダプタにはPOSIX sh、mktemp、mkfifoと対象アプリが必要。
Rust環境は実行時には不要。ここではglibc版Voidで検証し、musl版・他OSは未検証。

## 辞書

同梱の [data/demo.skk](data/demo.skk) は独自作成の小さなMITライセンスのデモ辞書。
実用的な語彙は外部辞書を指定する。

```sh
skk convert かんじ --dict /path/to/SKK-JISYO.L.utf8
export SKK_DICTIONARY=/path/to/SKK-JISYO.L.utf8
# EUC-JP版を利用する場合:
iconv -f EUC-JP -t UTF-8 SKK-JISYO.L > SKK-JISYO.L.utf8
```

`--dict` が環境変数より優先する。辞書指定時の読み込み失敗はエラーになり、
デモ辞書への暗黙のフォールバックはしない。候補なしのconvertは出力なし・終了コード0。
見出し語と `/候補;注釈/候補/` を読み、注釈は候補本文と分離する。
送りあり・ASCII/数値見出し語は読み飛ばす。重複候補は本文でまとめ、順番を保つ。
辞書のLisp式は評価しない。エスケープ式、数値変換、送りブロック等の完全なSKK辞書仕様には未対応。
辞書は起動時にメモリへ読み込む。ユーザー辞書への書き込み・学習はしない。
外部辞書のライセンスは配布元で確認する（[調査記録](docs/research.md)）。

## 入力操作

初期状態はひらがな。`ka → か`、`shi → し`、`konnichiha → こんにちは`。
促音、撥音、拗音、基本的な小書き文字と句読点に対応する。

|キー|動作|
|---|---|
|大文字 `K`|読みの入力開始。`Kanji → ▽かんじ`|
|Space|読みから検索。候補中では次候補|
|`x`|候補中では前候補。最初の候補からは読みに戻る|
|Enter|候補または読みを確定。空の直接入力では呼出元に返す|
|C-g / Escape|候補から読みへ戻る。読み・未確定ローマ字を取消し|
|Backspace|未確定ローマ字、読みをUnicode scalar単位で削除。直接入力では呼出元に返す|
|`q`|空の直接入力でひらがな/カタカナ切替。読み入力中では反対のかなで確定|
|`l` / C-j|空の直接入力でASCIIへ / ひらがなへ|

候補選択中の通常文字は現在候補を確定してから改めて処理する。
候補末尾のSpaceは末尾に留まり、未実装の登録機能をエラーで知らせる。
辞書未登録なら読みに留まり、Enterでかなを確定できる。
読み入力中の追加の大文字（送り開始）は未対応エラーで、現在の読みを保つ。

**互換性上の差異:** 要求例の `konnichiha` に合わせ、`nn` は次キーまで保留し、
続く母音・yで後半のnを使う。`nn<Enter> → ん`、`nni → んに`。
調査したlibskk/skkeletonの標準表は `nn` を直ちに `ん` にするため、ここは同一動作ではない。
`kon'nichiha` も使用可能。ローマ字表は基本セットであり、完全互換はまだ主張しない。
無効なローマ字は失わないよう文字のまま返す。この点も既存SKKとの比較拡充が必要。

## CLI・共通インターフェース

```sh
skk serve --stdio
# 入力: {"id":1,"key":"K"}
# 入力: {"id":2,"key":"a"}
```

各行に1応答を返す。`result.committed` はそのキーで確定した差分、
`result.preedit` は未確定表示全体で、実バッファへ挿入しない。
候補・インデックス・未処理キー・エラーも独立したフィールドに返す。
詳細と実例は [docs/protocol.md](docs/protocol.md)。
プロセスごとに1セッションで、ネットワーク・デーモン・グローバル入力状態はない。

Rustからは `Engine::process_key(Key, &Dictionary) -> Update` を利用できる。
エンジンはファイル、端末、Kakoune、tmuxを操作しない。

## 端末UI・tmux

```sh
skk tui
# Enterで変換確定、Ctrl-Dで全文を送信、Esc/Ctrl-Cで全文をキャンセル。
# 送信結果だけがstdoutへ出る。画面は/dev/ttyへ描画する。

install -m 755 adapters/skk-tmux "$HOME/.local/bin/skk-tmux"
# adapters/tmux.conf のbind-keyを ~/.tmux.conf に追加する。
```

tmuxのprefix + `J` でpopupを開く。`skk` と `skk-tmux` をtmux側のPATHに置く。
辞書環境変数は必要なら `tmux set-environment SKK_DICTIONARY /absolute/path` でも設定する。
元ペインの不変IDをpopup開始時に保存し、専用の一時bufferへ読み込み、対象へ貼る。
対象ペインが消えた場合はエラーとし、別ペインへ代替しない。
`paste-buffer -p` により、対応アプリではbracketed pasteを使用する。
テキストをシェルコマンドや `send-keys` の引数として解釈しない。
キャンセルはstdoutが空になるため何も貼り付けない。

TUIは全画面の簡単な行入力で、カーソル移動・再編集・候補一覧UI・リサイズ最適化は未実装。
Escは全体キャンセルなので矢印キーのESCシーケンスもキャンセルになる。
通常終了・I/Oエラー時には端末設定を復元するが、外部シグナルでの強制終了は未対応。

## Kakoune（実験的）

kakrcで `source /absolute/path/to/adapters/sumi.kak` を実行する。
Insert modeでCtrl-Kを押すとSKKを有効化する。
未確定文字列と選択中候補はステータス行へ表示し、確定文字列だけを挿入する。
Ctrl-Gで変換を取り消す。Escまたは再度Ctrl-KでSKKを終了しNormal modeへ戻る。
矢印キーもまずSKKを終了する（その移動自体は行わない）。

1セッションにつき1エンジンプロセスを起動し、windowのキーmapから直接FIFOへ送信する。
毎キーのシェル・新規プロセスはない。非表示FIFOバッファの通知hookが応答を適用する。
返信ファイルと1byte通知、ACKによって部分フレームや返信上書きを避ける。
高速入力中の終了もキューの後ろに流す。クライアント終了時にプロセスを終了させる。
バッファへ未確定文字列を仮挿入・巻き戻ししない。
複数選択は同じエンジンの確定文字列を各選択へ挿入する。

**制約:** 現状では通常編集を損なわない汎用IMEとして完成したとは扱わない。

- 有効中は対象文字のwindow mapを上書きし、終了時に削除する。既存のwindow mapを保存・復元しない。
- 捕捉対象は英字、数字、Space、Enter、Backspace、C-j、C-g、`.,-'`。
  その他の記号や編集キーの統一的な扱いは未完成。
- 有効中のマウス操作、バッファ/ウィンドウ切替、外部編集、補完、Undo/Redoとの相互作用は未検証。
  有効中に編集位置を外部から変更しないこと。
- 候補は現在選択だけを表示し、候補一覧・カーソル近傍の未確定表示はない。
- 単純な英数字・`_-` のクライアント名のみ。非常に大きな入力キューの回復処理は未完成。
- 異常終了時のmap復旧や一時ファイル回収はまだ堅牢化が必要。

普段のwindow mapを多用する環境では、専用windowで検証するかtmux popupを利用する。
Kakoune本体の改造は不要だが、上記の制約解消が本採用の条件となる。

## 構成と検証

```text
src/lib.rs          モード・変換段階・キー処理
src/romaji.rs       ローマ字変換
src/dictionary.rs   UTF-8 SKK辞書
src/protocol.rs     JSON Lines
src/main.rs         サブコマンド
src/terminal.rs     端末UI
src/kakoune.rs      Kakoune専用FIFOブリッジ
adapters/           Kakoune・tmuxの設定とスクリプト
tests/              状態遷移・辞書・通信・CLI・実端末統合
examples/latency.rs エンジン遅延測定
```

入力モード（ASCII/ひらがな/カタカナ）と変換段階（直接/読み/候補選択）を分離する。
検索は同期処理なので、持続するConverting状態は不要。
候補一覧と現在インデックスを同じ状態内に持たせ、無効なインデックスを外部から変更させない。
Rustはこの状態表現とUTF-8の安全性、単一バイナリ配布、テストの容易さから採用した。
詳細な言語比較と先行実装の調査は [docs/research.md](docs/research.md)。

```sh
cargo test --locked
cargo fmt --check
cargo clippy --all-targets -- -D warnings
sh tests/adapters.sh  # kak, tmux, rg, stty, cmpが必要。独立tmuxサーバー内で実行。
cargo run --release --example latency -- /path/to/SKK-JISYO.L.utf8
```

検証環境: Void Linux/glibc x86_64、Kakoune 2026.05.21、tmux 3.8。
9件のエンジン/通信テストと2件のCLIテスト、および実端末統合テストを用意した。
popupの開閉と確定/キャンセルは実際の接続クライアントでも確認した。
外部SKK-JISYO.Lの検索確認と測定結果は [docs/validation.md](docs/validation.md)。

## 続けて実装すること

1. ローマ字規則の互換モードと先行実装との比較テストを増やす。
2. Kakouneのmap保存・復元、編集位置とライフサイクルの堅牢化、入力遅延の実測。
3. 送り仮名、辞書形式の完全対応、ユーザー辞書・学習・登録。
4. TUIのキー解析・一覧表示・シグナル処理、Void muslでの配布検証。

コードと独自デモ辞書は [MIT](LICENSE)。先行実装のコード・辞書は同梱していない。
