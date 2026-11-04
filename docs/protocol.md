# stdio protocol v0 (MVP)

`skk serve --stdio` はプロセスごとに独立したひらがな状態を作る。
1つのUTF-8 JSONオブジェクトを1行で送り、同じ順番で1行のJSON応答を受け取る。
要求ごとにflushする。stdoutにログを混ぜない。EOFで終了し、状態は保存しない。
アプリごとにプロセスを起動する方式で、1プロセスの複数クライアント共有はしない。

```json
{"id":1,"key":"K"}
{"id":2,"key":"a"}
{"id":3,"key":"n"}
{"id":4,"key":"j"}
{"id":5,"key":"i"}
{"id":6,"key":"Space"}
{"id":7,"key":"Enter"}
```

要求のフィールドは `id` と `key` のみ。未知フィールドはエラー。
`id` は文字列または符号なし整数。同じIDでも要求は通常通り処理するため、再送の重複排除は呼出元の責務。
`key` は制御文字以外のUnicode scalar 1文字、または以下の名前。

|名前|意味|
|---|---|
|Space / ` `|空白|
|Enter|確定|
|Backspace|左削除|
|Escape|取消し（idleでは呼出元へ返却）|
|C-g|変換取消し|
|C-j|ひらがなモード|
|Apostrophe|`'` の別名（アダプタ用）|

改行文字や端末のバイト列を直接キーとして送らない。Enterを使用する。
初期モードの変更、辞書の再読み込み、候補番号指定、状態取得だけの要求は未実装。

6番目の要求の応答例:

```json
{"id":6,"result":{"mode":"hiragana","phase":"selecting_candidates","preedit":"▼漢字","committed":"","candidates":[{"text":"漢字","annotation":null},{"text":"感じ","annotation":"感覚"}],"candidate_index":0,"passthrough":null,"error":null}}
```

7番目の要求では `committed:"漢字"`、`preedit:""`、`phase:"direct"`、候補は空になる。

|resultのフィールド|定義|
|---|---|
|mode|`ascii` / `hiragana` / `katakana`|
|phase|`direct` / `composing` / `selecting_candidates`|
|preedit|その時点での未確定表示全体（▽/▼を含む）。差分ではない|
|committed|そのキーが確定した文字列の差分。重複適用しない|
|candidates|候補本文textと任意のannotation。選択中以外は空配列|
|candidate_index|0始まりの選択番号。選択中以外はnull|
|passthrough|エンジンが消費しなかった名前付きキー。呼出元の編集機能で処理する|
|error|辞書候補なし、未対応送り仮名などの入力操作エラー。通常はnull|

`result.error` があっても、その時点の状態がresultに含まれる。
操作によっては末尾nのかな化等が行われるため、エラーを理由にresult全体を破棄しない。
確定差分と未処理キーの両方がある場合は、確定を先に適用する。
候補注釈は表示専用で、確定文字列へ含めない。

入力形式が不正な場合は `result` の代わりにトップレベルの `error` を返す。
この場合はエンジン状態を変更しない。

```json
{"id":8,"error":{"code":"invalid_key","message":"expected a Unicode character or named key"}}
```

コードは `invalid_json`、`invalid_request`、`invalid_key`、`invalid_utf8`、`frame_too_large`。
形式上取得できたIDは返却する。JSON自体が壊れている場合などは `id:null`。
メッセージ本文は診断用で安定したAPIではない。
1行はLFを含め最大65,536byte。超過フレームは改行まで読み捨て、次の行から回復する。
途中の不正UTF-8もエラー応答を返し、セッションを継続する。
入出力のI/Oエラーはstderrへ診断して非0終了する。

このプロトコルはMVP版で、拡張時の互換性方針やバージョンネゴシエーションは今後定義する。
Kakoune用のFIFOはアダプタ内部実装であり、この公開プロトコルとは別である。
