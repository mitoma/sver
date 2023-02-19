# 同じビルドやテストを何度も実行しないための GitHub Actions

前回 [同じビルドやテストを何度も実行しない方法][] として GitHub Actions での実現方法や [sver][] というコマンドラインツールを紹介しました。

今回、汎用的に使える部分を [sver-actions][] として切り出したので紹介します。

内部動作の詳細については [同じビルドやテストを何度も実行しない方法][] で書かれたもののままです。仕組みが気になる方は前回の記事を参照ください。

## 使い方

[sver-actions][] は 2 つの Action に分かれています。

一つは sver をセットアップするための `mitoma/sver-actions/setup@v1`。
もう一つは sver を使ってあるバージョンのジョブの実行が一度成功すれば次回からスキップする `mitoma/sver-actions/exec@v1` です。

### Setup action

セットアップするアクションは簡単です。
典型的には step に以下のように書いておけばこのステップ以降 sver を使ってバージョン計算をすることができます。
デフォルトでは linux にその時の最新バージョンの sver をインストールし、パスを通します。

```yaml
- uses: mitoma/sver-actions/setup@v1
```

linux 以外の OS を使いたかったり、特定のバージョンをインストールしたいときには以下のように指定できます。

```yaml
- uses: mitoma/sver-actions/setup@v1
  with:
    # linux, windows, macos のいずれかを指定することができます
    os: windows
    # お好きなバージョンをインストールすることができます
    version: v0.1.14
```

### Exec action

実行するアクションは少し設定が必要です。

このアクションを実行する前に `mitoma/sver-actions/setup@v1` で [sver][] をインストールしておく必要があります。

実行したい内容 `command` と、その結果実行結果を GitHub Actions の artifact に保存するために `phase` と `github_token` を渡す必要があります。

設定例を示します。

```yaml
- uses: mitoma/sver-actions/exec@v1
  with:
    phase: build
    github_token: ${{ secrets.GITHUB_TOKEN }}
    command: |
      cargo build
```

上記のように指定すると初回のジョブでは cargo build を実行し、成功すれば `{phase}-{version}.success` 形式の 0 バイトのファイルを artifact としてアップロードします。例でいうと `build-18b280c304ab.success` といった感じの名前になります。

そして初回以降、 `{phase}-{version}.success` という名前の一致する artifact が見つかる限りこのジョブはスキップされます。


ジョブを実行するかどうかをある特定のディレクトリ以下やファイル群の変更に限りたい場合には `path` を使ってバージョンの計算対象を変更することができます。ライブラリの依存関係やジョブの性質によって詳細に対象を制御したい場合には [sver][] の sver.toml を記述して設定します。

また、ビルド時のキャッシュの利用やビルド結果を artifact にアップロードする処理もこのアクションで制御することができます。

以下は指定できるパラメータの設定例です。このパラメータの説明など詳細は [sver-actions/exec][] を参照いただければと思います。
(基本的には内部で `actions/cache/save`, `actions/cache/restore`, `actions/upload-artifact` を呼び出しているだけです)

```yaml
# standard rust project example.
- uses: mitoma/sver-actions/exec@v1
  with:
    # ビルドのフェイズ。リポジトリの CI の中で一意な名前をつける。
    phase: build
    # ビルド時に artifact をアップロードするため、権限のある TOKEN を指定する必要がある。
    github_token: ${{ secrets.GITHUB_TOKEN }}
    # ジョブのバージョンを計算対象とするパスを指定する。
    path: .
    # 実行するジョブの内容を書く。複数行書いてもよい。
    command: |
      cargo build --release
    # キャッシュを restore だけでなく save するときに true を指定する。デフォルト true。
    # 例ではデフォルトブランチの時のみキャッシュを保存する。
    cache_save_enable: ${{ github.ref == format('refs/heads/{0}', github.event.repository.default_branch) }}
    # `actions/cache/save` の `key` と同様。
    cache_key: cargo-${{ hashFiles('**/Cargo.lock') }}
    # `actions/cache/save` の `restore-key` と同様。
    cache_restore-keys: |
      cargo-${{ hashFiles('**/Cargo.lock') }}
      cargo-
    # `actions/cache/save` の `path` と同様。
    cache_path: |
      ~/.cargo/registry
      ~/.cargo/git
      target
    # `actions/upload-artifact` の `name` と同様。ただし、末尾に `-{version}` が付与される。
    artifact_name: build-result
    # `actions/upload-artifact` の `path` と同様。
    artifact_path: path/to/artifact
```

以上、本アクションが皆さんの快適な CI ライフの一助となれば幸いです。

[同じビルドやテストを何度も実行しない方法]: https://mitomasan.hatenablog.com/entry/2022/07/15/080000
[sver]: https://github.com/mitoma/sver
[sver-actions]: https://github.com/mitoma/sver-actions
[sver-actions/exec]: https://github.com/mitoma/sver-actions/tree/main/exec
