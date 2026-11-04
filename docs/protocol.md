# stdio protocol version 1

`skk serve --stdio` は、1 プロセスにつき独立した入力セッションを提供します。
UTF-8 の JSON オブジェクトを 1 行ずつ標準入力へ送り、同じ順番で 1 行ずつ応答を受け取ります。
各応答を flush し、標準出力へログを混ぜません。
EOF でセッションを終了し、未確定入力を破棄します。
ユーザー辞書への保存は `--user-dict` を指定した場合だけ行います。

初期モードはひらがな、ローマ字規則は標準 SKK です。
`--romaji common` で、二重の n を保留する規則へ変更できます。
複数の入力状態が必要な場合は、別々のプロセスを起動します。
`id` は要求と応答の対応付けに使い、セッションを識別するものではありません。

## バージョン

全応答のトップレベルに `version:1` を含めます。
要求にも `version:1` を指定でき、省略した場合は version 1 として扱います。
未対応のバージョンは `unsupported_version` エラーとし、入力状態を変更しません。

既存フィールドの意味や型を壊す変更は、別のプロトコルバージョンで扱います。
応答フィールドや操作の追加は互換な拡張として扱うため、クライアントは未知の応答フィールドを無視してください。
サーバーは未知の要求フィールドや操作をエラーにします。

## 要求

`id` は文字列または符号なし整数です。
同じ ID の再送も新しい操作として処理するため、重複排除や再送判断は呼出元が行います。

|op|追加フィールド|動作|
|---|---|---|
|key（省略可）|key|キーを入力|
|state|なし|現在の状態を取得|
|reset|なし|未確定入力を破棄。モードとローマ字規則を保つ|
|commit|なし|現在の候補または読みを確定。idle では何もしない|
|select|index|0 始まりの候補番号を選ぶ。確定は別操作|
|register|reading, text, 任意の okuri|単語を登録。現在の未確定入力を保つ|

```json
{"version":1,"id":1,"key":"K"}
{"id":2,"op":"key","key":"a"}
{"id":3,"op":"state"}
{"id":4,"op":"reset"}
```

各操作で定義したフィールドだけを送ります。
たとえば `reset` に `key` や `index` を指定すると `invalid_request` になります。

`key` は制御文字以外の Unicode scalar 1 文字、または次の名前です。
改行や端末のエスケープシーケンスを直接送らず、名前付きキーを使います。

|名前|意味|
|---|---|
|Space または半角空白|検索、候補移動、直接入力の空白|
|Enter|確定。空の直接入力では呼出元へ返す|
|Backspace|未確定文字の削除。空の直接入力では呼出元へ返す|
|Escape / C-g|変換の取消し|
|C-j|確定してひらがなモードへ|
|Apostrophe|`'` の別名|

## 応答

正常な要求には `result` を返します。
`committed` はその要求で確定した差分、`preedit` はその時点の未確定表示全体です。
呼出元は `committed` を一度だけ挿入し、`preedit` を表示用として別に管理します。

|result のフィールド|定義|
|---|---|
|mode|`ascii` / `hiragana` / `katakana`|
|romaji_rule|`skk` / `common`|
|phase|`direct` / `composing` / `selecting_candidates`|
|preedit|`▽` や `▼` を含む未確定表示全体|
|pending|未確定ローマ字|
|reading|ひらがなの語幹。直接入力では null|
|okuri|完成した送り仮名。送り入力の開始時は空文字列、送りなしでは null|
|dictionary_key|送りキーの英字を含む見出し語。直接入力では null|
|committed|この要求による確定差分|
|candidates|本文 text と任意の annotation。選択中以外は空配列|
|candidate_index|0 始まりの選択番号。選択中以外は null|
|passthrough|呼出元で処理する名前付きキー。通常は null|
|error|候補なし、候補番号の範囲外などの操作エラー。通常は null|
|confirmed|確定した辞書候補の情報。それ以外は null|
|learning_error|ユーザー辞書への保存失敗。通常は null|

`state` は確定差分を返さず、過去の確定を繰り返し通知しません。
`commit` は idle で何もしませんが、`key:Enter` は idle なら `passthrough:Enter` を返します。
確定差分と未処理キーが両方ある場合は、確定差分を先に適用します。

## 変換

次の入力で、読みから候補選択、確定まで進みます。

```json
{"id":1,"key":"K"}
{"id":2,"key":"a"}
{"id":3,"key":"n"}
{"id":4,"key":"j"}
{"id":5,"key":"i"}
{"id":6,"key":"Space"}
{"id":7,"op":"select","index":1}
{"id":8,"op":"commit"}
```

デモ辞書では最初の候補が「漢字」、2 番目が「感じ」で、最後の確定差分は「感じ」です。
候補番号が範囲外なら `result.error` を返し、現在の候補を保ちます。
注釈は表示用の情報で、確定文字列へは含めません。

送り仮名付きの `O k u R u` は、Space を待たず `おくr` を検索します。
このとき `reading` は `おく`、`okuri` は `る` です。
候補の本文は語幹だけで、`preedit` と確定差分には送り仮名も含めます。
Ctrl-G で候補から読みに戻った場合も、送り仮名を保持します。

## 登録と学習

`register` は、`reading` に見出し語、`text` に語幹を指定します。
登録は次の検索から有効になり、現在の読みや候補選択は変更しません。

```json
{"id":9,"op":"register","reading":"おくr","text":"贈","okuri":"る"}
```

ユーザー辞書を指定しなければ、登録はセッション内だけに残ります。
指定した場合は登録を保存し、辞書候補の確定時にも優先順位を学習します。
保存に失敗しても、セッション内の登録や確定文字列は有効で、`learning_error` に理由を返します。
呼出元は、本文を渡す処理と保存失敗の表示を分けて扱います。

`confirmed` は次の形式です。
純粋な Engine を利用する呼出元でも、独自の学習処理に使えます。

```json
{"dictionary_key":"おくr","candidate":{"text":"贈","annotation":null},"okuri":"る"}
```

Rust の低水準 API `protocol::respond` は読み取り専用辞書を受け取るため、登録できません。
登録には `respond_session` または `serve_session` を使います。
CLI の `skk serve --stdio` は、全操作に対応した Session を使います。

## エラーと上限

入力形式が不正な場合は `result` の代わりにトップレベルの `error` を返します。
この場合は入力状態を変更しません。

```json
{"version":1,"id":10,"error":{"code":"invalid_key","message":"expected a Unicode character or named key"}}
```

エラーコードは `invalid_json`、`invalid_request`、`invalid_key`、`invalid_utf8`、`frame_too_large`、`unsupported_version` です。
取得できた ID は応答に含め、JSON 自体が壊れている場合などは `id:null` にします。
メッセージ本文は診断用で、文字列の内容を安定した API として扱いません。

要求の 1 行は LF を含め最大 65,536 byte です。
超過行は次の改行まで読み捨て、不正 UTF-8 と同様に、エラー応答後もセッションを継続します。
最後の行の LF は省略できます。
応答サイズは候補数や本文の長さに依存します。

`result.error` や `learning_error` がある場合も、返した状態と確定差分は有効です。
たとえば検索失敗の前に、末尾の n がかなへ変わることがあります。
エラーを理由に `result` 全体を破棄しないでください。

入出力のエラーは stderr に診断して非 0 終了します。
出力先が閉じた場合の Broken pipe は、通常の Unix パイプで使えるよう正常終了にします。
辞書の再読み込みと、複数セッションの多重化は提供しません。
