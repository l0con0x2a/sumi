# sumi

sumi は、Unix のコマンドやアプリケーションと組み合わせて使う SKK 日本語入力エンジンです。
Rust ライブラリ `sumi_skk` と、単体で使える `skk` コマンドを提供します。
OS の IME や、特定のエディタ、ターミナルマルチプレクサは必要ありません。

## 試す

Rust stable と C リンカを用意し、ビルドします。
検証環境は Void Linux/glibc、Rust 1.99.1 です。

```sh
cargo build --release --locked
./target/release/skk kana konnichiha
# こんにちは
./target/release/skk convert かんじ
# 漢字
# 感じ
```

インストールする場合は次を実行します。

```sh
cargo install --path . --locked
```

Rust はビルド時だけ必要です。
直接依存は JSON 通信に使う `serde` と `serde_json` です。
辞書とローマ字変換のロジックにエディタ依存や OS IME 依存はありません。

## かな変換

`skk kana` はローマ字をかなに変換するフィルタです。
大文字を変換開始キーとして扱わず、小文字と同じローマ字として処理します。
ひらがなとカタカナに対応します。

```sh
skk kana Kanji
# かんじ
skk kana --katakana nihon
# ニホン
printf 'konnichiha\nnihon' | skk kana
```

引数を省略すると UTF-8 の標準入力を読みます。
入力の改行や既存の日本語は保ち、EOF で末尾の `n` を「ん」に確定します。
引数を指定した場合だけ出力末尾に改行を付けます。
未完成の `ky` などは文字列のまま残します。
このコマンドは辞書を読み込みません。

## 辞書検索

`skk convert` は見出し語から候補を検索し、本文を1行ずつ返します。
同梱のデモ辞書は動作確認用の小さな独自辞書です。
普段の入力には、外部の UTF-8 SKK 辞書を指定します。

```sh
skk convert --dict /path/to/SKK-JISYO.L.utf8 かんじ
skk convert --json かんじ
export SKK_DICTIONARY=/path/to/SKK-JISYO.L.utf8
```

`--dict` は複数回指定できます。
先に指定した辞書を優先し、同じ候補は本文で重複を除きます。
JSON 出力には本文 `text` と注釈 `annotation` を含めます。
送り仮名ブロックは、その送り仮名に合う候補を優先する情報として読み込みます。
`(concat "...")` の文字列表現と8進エスケープはデータとして復号し、任意の Lisp は実行しません。
`skk check-dict FILE` で構文を検証し、読み込んだ見出し語や対応外データの件数を JSON で確認できます。

```sh
skk convert --dict personal.skk --dict SKK-JISYO.L.utf8 かんじ
iconv -f EUC-JP -t UTF-8 SKK-JISYO.L > SKK-JISYO.L.utf8
```

`--dict` を省略した場合は `SKK_DICTIONARY` を使い、どちらもなければデモ辞書を使います。
指定した辞書が読めなければエラーにし、別の辞書へ暗黙に切り替えません。
候補がなければ出力なしで正常終了します。
外部辞書は配布元のライセンスを確認してください。
同梱のデモ辞書とコードは [MIT](LICENSE) です。

## 対話入力

`skk tui` は端末で日本語を入力し、完成した文字列を標準出力に返します。
画面の表示とキー入力には `/dev/tty` を使います。
画面の制御文字や未確定文字列は標準出力へ混ぜません。

```sh
skk tui
skk tui > message.txt
```

初期状態はひらがなです。
`Kanji` で `▽かんじ` を作り、Space で候補選択、Enter で確定します。
送り仮名の先頭を大文字にすると、かなが完成した時点で検索します。
たとえば `OkuRu` で `▼送る`、`KaKu` で `▼書く` になります。
Ctrl-D で全文を送信し、Esc/Ctrl-C で全体をキャンセルします。
キャンセルした場合は標準出力に何も返しません。

|キー|動作|
|---|---|
|大文字|変換する読みの入力を開始|
|Space|検索、または次候補|
|`x`|前候補。最初の候補からは読みに戻る|
|Enter|候補または読みを確定。空の直接入力では改行|
|Ctrl-G|候補から読みに戻る、または読みを取り消す|
|Backspace|未確定ローマ字、読み、確定した本文の順に左削除|
|`q`|空の直接入力でひらがなとカタカナを切り替える|
|`l` / Ctrl-J|空の直接入力で ASCII へ / ひらがなへ|

端末 UI は開始と終了のときだけ `stty` を起動します。
現状は簡単な末尾入力で、本文中のカーソル移動や再編集は未対応です。
外部シグナルによる強制終了後の端末復元も今後の課題です。

## 他のプログラムから使う

`skk serve --stdio` は、1プロセスにつき独立した入力セッションを提供します。
標準入力にキーイベントを JSON Lines で送り、応答を標準出力から読みます。
毎キーのプロセス起動や、常駐デーモンは必要ありません。

```sh
skk serve --stdio
```

```json
{"id":1,"key":"K"}
{"id":2,"key":"a"}
```

応答では、確定した差分 `committed` と未確定表示全体 `preedit` を分けます。
候補、選択番号、呼出元で処理するキー、エラーも別のフィールドで返します。
[通信仕様](docs/protocol.md) に要求と応答の定義を記載しています。

Rust からは `Engine::process_key(Key, &Dictionary) -> Update` を使えます。
かな変換だけなら `kana::Converter` を使い、入力の断片を順に渡せます。
エンジンは画面表示や外部プロセスを操作しません。

Kakoune や tmux 向けの実験的な連携は、隣の独立リポジトリ `../sumi-adapters` へ移しました。
このリポジトリのビルドや通常のテストには必要ありません。
アダプタは公開された stdio 通信または CLI の標準出力を使います。

## 制約

現時点では基本的なかな入力、送りなし変換、送り仮名付き変換に対応しています。
ユーザー辞書への保存、学習、登録、abbrev、数値変換は未実装です。

エンジン、`serve`、`tui` は、標準 SKK と同じく `nn` を直ちに「ん」に確定します。
「こんにちは」は `kon'nichiha` と入力できます。
`kana` は一般的なローマ字入力向けに、後半の `n` を保留して `konnichiha` を受け付けます。
`--romaji skk` または `--romaji common` で、使う規則を明示できます。

```sh
skk kana --romaji skk "kon'nichiha"
skk serve --stdio --romaji common
```

外来音の `thi`、`twu`、`tsa` などと、`zh`、`zj`、`zk`、`zl` による矢印入力にも対応します。
無効なローマ字や未完成の入力は失わないよう文字のまま残します。
ローマ字表の拡充と、既存 SKK との比較は継続する課題です。

引数の誤りは終了コード 2、辞書や I/O のエラーは 1、正常終了は 0 です。
出力先が閉じた場合の Broken pipe は正常終了として扱います。
`--help` と `--version` は辞書を読み込みません。

## 開発

```sh
cargo test --locked
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo run --release --example latency -- /path/to/SKK-JISYO.L.utf8
```

|ファイル|責務|
|---|---|
|`src/lib.rs`|SKK のモードと変換状態|
|`src/romaji.rs`|ローマ字規則|
|`src/kana.rs`|辞書を使わない逐次かな変換|
|`src/dictionary.rs`|UTF-8 辞書の読み込みと検索|
|`src/protocol.rs`|stdio セッション|
|`src/main.rs`|CLI とフィルタ|
|`src/terminal.rs`|単体の端末 UI|
|`tests/`|エンジン、辞書、通信、CLI の自動テスト|

Rust を採用した理由、先行実装の参照先、ライセンス確認は [調査記録](docs/research.md) に残しています。
初期実装時の実機確認と遅延測定は [検証記録](docs/validation.md) を参照してください。
