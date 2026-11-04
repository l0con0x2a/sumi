# 調査と設計判断

以下は初期実装時の調査記録です。
アプリケーション連携の試作は現在 `../sumi-adapters` リポジトリで管理し、本体のコードやテストには含めていません。

調査日: 2026-11-05。リポジトリは当初空で、既存コードの移植は行っていない。
先行実装を一時ディレクトリへ取得し、状態管理・規則・テスト・ライセンスを参照した。
以下のコミットは参照対象の固定記録であり、ビルド時の依存ではない。

|対象|調査コミット|参照した主な箇所|
|---|---|---|
|[skkeleton](https://github.com/vim-skk/skkeleton)|`e78aca14eccc3c5b019449e60d356cf8d5c3ded2`|`kana/rom_hira.ts`, `preedit.ts`, `dictionary.ts`, `okuri.ts`, 対応する`*_test.ts`|
|[skk.nvim](https://github.com/nabehan/skk.nvim)|`d3dd0a3857dc7f95156ab9e7c9d4d89ce1f300a7`|henkanのstate/session、`session_spec.lua`, `state_spec.lua`, `candidate_nav_spec.lua`, `jisyo_parser_spec.lua`, `user_dict_spec.lua`|
|[libskk](https://github.com/ueno/libskk)|`147beaf409eb119df2d005a5927ad2c7143a8c82`|`tests/rom-kana.c`, `basic.c`, `context.c`, `file-dict.c`, `user-dict.c`, `rules/default/rom-kana/default.json`|
|[SKK辞書](https://github.com/skk-dev/dict)|実ファイルのSHA-256はvalidation.md|`committers.md`, SKK-JISYO.L|

## SKKの動作から取り入れた点

- ASCII、ひらがな、カタカナ等の入力モードと、直接入力・見出し入力・候補選択は別の軸。
  エディタのInsert/Normal modeとは結合させない。
- ローマ字変換は未確定の接頭辞を保持し、かな出力と分離する。促音では次の子音を残す。
  nは母音/yとの結合があり、確定時と次子音時の処理が必要。
- 大文字で見出し入力を開始し、もう一度の大文字が送り仮名境界になる。
  MVPは後者をエラーとして明示し、送りあり変換を装わない。
- 候補から取消すと読みへ戻り、読みから取消すと直接入力へ戻る。
  読みは候補選択中も失わない。候補移動と確定を分離する。
- `x`で前候補、通常文字で候補確定後に文字を処理する。候補末尾では本来登録へ進む実装があるが、
  MVPは登録未対応のエラーで現在候補を保つ。
- 見出しと候補列は辞書順序を保つ。候補本文と注釈を分ける。
  学習は候補の優先順位とユーザー辞書の更新を伴うため、現在は永続化を導入しない。
- skkeletonのpreeditテストは表示済み文字列の差分や書記素を検証する。
  sumiではUIによる削除・再挿入をエンジンへ持ち込まず、確定差分と未確定全体を返す。
- skk.nvimのsessionテストはpendingを優先したBackspace、UTF-8読み削除、送り境界等を扱う。
  libskkのrom-kanaテストは途中の子音、nの確定、促音、句点等を扱う。
  これらを観点として独自の状態遷移テストを作った。上流テストの完全移植・差分実行はしていない。

未実装: 半角カナ、全角英数、abbrev、数値、送り仮名、補完、入れ子登録、ユーザー辞書、学習。
標準表の `nn` と今回の要求例の違いはREADMEに記した。
SKKは実装・規則設定ごとの差があり、限られたテストだけで完全互換とは結論しない。

## ライセンスと再利用

各プロジェクトのLICENSE/COPYINGおよび辞書の配布条件を確認した。

- skkeleton: zlib型の条件（出自の偽装禁止、変更の明示、通知の保持）。
- skk.nvim: MIT（通知保持）。
- libskk: GPL-3.0-or-later。ライブラリやコードを組み込む場合はGPL条件の検討が必要。
- [辞書のcommitters.md](https://github.com/skk-dev/dict/blob/master/committers.md):
  S/M/L等はGPL-2.0-or-later。pubdic+、edict、emoji、地名辞書等はそれぞれの条件があり、一括で同じ条件とは扱えない。

sumiは上流のソース・変換表・辞書をコピーして同梱していない。
独自コードとデモ辞書をMITで提供し、一般辞書は利用者が外部ファイルとして指定する。
外部辞書や上流コードを将来再配布する際は、そのファイルに適用される条件を改めて確認する。
依存クレートはCargo.lockに固定。直接依存のserde/serde_jsonはMIT OR Apache-2.0。

## 言語比較

|基準|Rust（採用）|C|Haskell|
|---|---|---|---|
|状態機械|enumとmatchで候補状態のデータをまとめられる|tagged unionと手動の不変条件管理|代数的データ型で自然に表現可能|
|UTF-8|Stringの境界検査とchars|文字境界・メモリ管理を手作業|Textで扱えるが標準Stringとの使い分けが必要|
|低レイテンシ|GCなし、起動後の処理が小さい|同等の制御が可能|GC/ランタイム調整の評価が必要|
|CLI/配布|単一バイナリ、Cargoと少数依存|小さなバイナリ、JSON等の実装選定が必要|GHCと依存のビルド環境が相対的に重い|
|テスト・保守|組み込みテスト、所有権による検証|テスト基盤とメモリ検証が追加で必要|純粋状態遷移のテストに適する|

Haskellが不適切という意味ではないが、今回の少依存・Unix配布・保守の組み合わせで
Rustから変更する強い理由はなかった。実行時依存とツールチェーン依存は区別する。

## Kakouneの技術検証

[公式hooks](https://github.com/mawww/kakoune/blob/master/doc/pages/hooks.asciidoc)、
[commands](https://github.com/mawww/kakoune/blob/master/doc/pages/commands.asciidoc)、
[expansions](https://github.com/mawww/kakoune/blob/master/doc/pages/expansions.asciidoc)、
mapping/buffers/modesと、インストール済み2026.05.21のドキュメントを確認した。

`InsertChar`/`InsertKey`だけでの透過変換は編集済みバッファとの調停が必要。
`on-key`とcommand/response FIFOを同期シェル内で使う初期試作では、
シェル待機中に実端末の入力を処理できず、成立しなかった。
`kak -p`も全stdinのEOF後に送信するため、1プロセスを入力ストリームとして使う方式にはしなかった。

採用した実験方式はInsert map → キーFIFO → 常駐アダプタ → 返信ファイルと通知FIFO → BufReadFifo。
`echo -to-file`は改行を付けないため、専用の改行optionを明示的に付加した。
単一byte通知とACKでフレームの部分読込みとファイル上書きを分離する。
コマンド生成はアダプタ内だけに置き、文字列はKakoune用に引用し、`<`のキー記法も逃がす。
単純なクライアント名と固定テンプレートのprivateディレクトリに限定する。

2選択への挿入と高速入力終了を実機で自動確認できた。
一方、既存window map復元、外部からの編集位置変更、補完等は未解決で、透過IMEとしては未完成。
実用性を優先する代替はtmux popup。Kakoune promptで一括変換する方式は統合が簡単だが、
Insert modeでの逐次かな入力という今回の操作目標を満たさない。PTYラッパーはTUI汎用だが
アプリ固有の編集キーを透過的に扱う責務が大きく、MVPには採用しなかった。

## tmuxの技術検証

[公式tmux.1](https://github.com/tmux/tmux/blob/master/tmux.1)とローカル3.8で確認した。
`display-popup -E`でTUIを実行し、`load-buffer`と`paste-buffer`で確定結果を移送する。
CLI引数長や末尾改行の問題を避けるため、`set-buffer`でテキスト引数を渡す代わりに
privateファイルから`load-buffer`する。文字列を`send-keys`で再解釈しない。

検証では `display-popup` のshell-command中の `#{pane_id}` は展開されなかった。
設定例は `run-shell` で呼出時にformat展開してからpopupを開く。
ペインIDは生存期間中に変わらず、貼り付け時にactive paneを再検索する必要がない。
接続クライアントで実際のpopup、確定貼り付け、キャンセルを確認した。
