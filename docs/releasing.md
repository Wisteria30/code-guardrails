# Releasing

## Quick Start

```bash
# 1. バージョンを同期（Cargo.toml, plugin.json, marketplace.json）
scripts/release.sh 0.2.0

# 2. コミット & PR
git add -A && git commit -m "release: v0.2.0"
git push origin HEAD

# 3. PR をマージ → 自動で GitHub Release が作成される
```

## 仕組み

### バージョンの管理

バージョンは3箇所に定義されている:

| ファイル | 役割 |
|---|---|
| `Cargo.toml` | バイナリに埋め込まれる（`--version` で出力） |
| `.claude-plugin/plugin.json` | SessionStart でバイナリとの整合性チェックに使用 |
| `.claude-plugin/marketplace.json` | マーケットプレイス表示用（バイナリとの連動は不要） |

`Cargo.toml` が唯一の真実の源。`scripts/release.sh` で3ファイルを一括更新する。

### CI による保護（PR 時）

`version-lint` ジョブが以下を検証:

1. **バージョンバンプ必須チェック**: `src/`, `hooks/`, `rules/`, `commands/` 等に変更があるのに `Cargo.toml` のバージョンが上がっていなければ fail
2. **バージョン一致チェック**: `Cargo.toml` と `plugin.json` のバージョンが不一致なら fail

### CD（main マージ時）

`release.yml` が以下を自動実行:

1. `Cargo.toml` のバージョン変更を検知
2. 4プラットフォーム（macOS aarch64/x86_64, Linux aarch64/x86_64）でバイナリをビルド
3. `v{version}` タグを作成し GitHub Release を公開

手動でのタグ作成は不要。

### ユーザー側の自動更新

`session-start.sh` が Claude Code セッション開始時に実行:

1. `plugin.json` のバージョンとバイナリの `--version` を比較
2. 不一致ならバイナリを削除して `setup` を再実行
3. `setup` は `plugin.json` のバージョンに一致するリリースからバイナリをダウンロード

## フロー図

```
scripts/release.sh 0.2.0
  ↓
Cargo.toml = 0.2.0
plugin.json = 0.2.0
marketplace.json = 0.2.0
  ↓
PR 作成 → CI: version-lint pass
  ↓
main にマージ
  ↓
CD: バージョン変更検知 → ビルド → v0.2.0 タグ → GitHub Release
  ↓
ユーザーがプラグイン更新 → plugin.json が 0.2.0 に
  ↓
Claude Code セッション開始 → session-start.sh
  ↓
バイナリ 0.1.0 ≠ plugin.json 0.2.0 → setup 再実行 → v0.2.0 ダウンロード
```
