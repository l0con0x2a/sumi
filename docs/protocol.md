# stdio protocol v0 (MVP)

`skk serve --stdio` はプロセスごとに独立したひらがな状態を作る。
既定のローマ字規則は標準 SKK の `nn → ん` で、`--romaji common` なら後半の `n` を保留する。
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

キー入力では `id` と `key` を指定する。`op:"key"` を明示してもよい。
他の操作は `op` に種類を指定する。未知フィールドや、その操作と無関係なフィールドはエラー。
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
|op|追加フィールド|動作|
|---|---|---|
|key（省略可）|key|キー入力|
|state|なし|状態を取得。確定差分は空で、入力状態を変えない|
|reset|なし|未確定入力を破棄。モードとローマ字規則を保つ|
|commit|なし|現在の候補または読みを確定。idle では改行を返さない|
|select|index|0始まりの候補番号を選ぶ。確定は別操作|

```json
{"id":8,"op":"state"}
{"id":9,"op":"select","index":1}
{"id":10,"op":"commit"}
```

候補番号が範囲外の場合は `result.error` を返し、現在の候補を保つ。
初期モードの変更と辞書の再読み込みは未実装。

6番目の要求の応答例（主要フィールドのみ）:

```json
{"id":6,"result":{"mode":"hiragana","phase":"selecting_candidates","preedit":"▼漢字","committed":"","candidates":[{"text":"漢字","annotation":null},{"text":"感じ","annotation":"感覚"}],"candidate_index":0,"passthrough":null,"error":null}}
```

7番目の要求では `committed:"漢字"`、`preedit:""`、`phase:"direct"`、候補は空になる。

|resultのフィールド|定義|
|---|---|
|mode|`ascii` / `hiragana` / `katakana`|
|romaji_rule|`skk` / `common`。セッションの起動時に指定した規則|
|phase|`direct` / `composing` / `selecting_candidates`|
|preedit|その時点での未確定表示全体（▽/▼を含む）。差分ではない|
|pending|未確定のローマ字。表示の解析をせずに取得できる|
|reading|ひらがなの語幹。直接入力ではnull|
|okuri|確定した送り仮名のかな。送り入力中は空文字列、送りなしではnull|
|dictionary_key|送りキーの英字を含む見出し語。直接入力ではnull|
|committed|そのキーが確定した文字列の差分。重複適用しない|
|candidates|候補本文textと任意のannotation。選択中以外は空配列|
|candidate_index|0始まりの選択番号。選択中以外はnull|
|passthrough|エンジンが消費しなかった名前付きキー。呼出元の編集機能で処理する|
|error|辞書候補なし、未対応送り仮名などの入力操作エラー。通常はnull|
|confirmed|辞書候補を確定した場合の見出し語、候補、送り仮名。それ以外はnull|
|learning_error|任意のユーザー辞書への保存失敗。確定文字列は有効なまま返す。通常はnull|

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
クライアントは追加された応答フィールドを無視できるようにする。

`--user-dict FILE` を付けて起動した場合、辞書候補の確定時に学習する。
保存失敗は `learning_error` として返し、セッションを終了しない。
確定した本文をアプリケーションへ渡す処理と、保存失敗の表示を分けて扱う。
`confirmed` は次のようなデータで、純粋な Engine を利用する呼出元でも独自の学習処理に使える。

```json
{"dictionary_key":"おくr","candidate":{"text":"贈","annotation":null},"okuri":"る"}
```

送り仮名付き入力では、たとえば `O k u R u` で見出し語 `おくr` を検索する。
かなが完成すると Space を待たず候補選択へ進む。
`reading` は `おく`、`okuri` は `る`、`dictionary_key` は `おくr` となる。
候補配列の本文は語幹だけで、`preedit` と確定差分には送り仮名も含む。
Ctrl-G で候補から読みに戻った場合も、送り仮名を保持する。
