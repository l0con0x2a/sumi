# 外部依存

Cargo.lock に固定した依存のライセンス宣言を記載します。
この一覧は各クレートのライセンス本文の代替ではありません。
再配布時には、対象バージョンの配布物に含まれる通知と条件を確認してください。

|直接依存|用途|ライセンス宣言|
|---|---|---|
|serde 1.0.229|公開状態のシリアライズ|MIT OR Apache-2.0|
|serde_json 1.0.154|JSON Lines と JSON 検索出力|MIT OR Apache-2.0|
|libc 0.2.191|任意の端末機能の POSIX API|MIT OR Apache-2.0|
|unicode-width 0.2.3|任意の端末機能の表示幅|MIT OR Apache-2.0|
|unicode-segmentation 1.13.3|任意の端末機能の結合文字編集|MIT OR Apache-2.0|

|間接依存|ライセンス宣言|
|---|---|
|itoa 1.0.18|MIT OR Apache-2.0|
|memchr 2.8.3|Unlicense OR MIT|
|proc-macro2 1.0.107|MIT OR Apache-2.0|
|quote 1.0.47|MIT OR Apache-2.0|
|serde_core 1.0.229|MIT OR Apache-2.0|
|serde_derive 1.0.229|MIT OR Apache-2.0|
|syn 3.0.9|MIT OR Apache-2.0|
|unicode-ident 1.0.27|(MIT OR Apache-2.0) AND Unicode-3.0|
|zmij 1.0.23|MIT|

外部の SKK 辞書は同梱していません。
独自のデモ辞書は本体と同じ MIT ライセンスです。
先行実装と SKK 辞書の調査記録は `docs/research.md` を参照してください。
