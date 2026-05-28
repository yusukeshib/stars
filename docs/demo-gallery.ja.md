# デモギャラリー

`stars` のキュレーション済みツアー — 肉眼〜小型望遠鏡で観測可能な天文現象を
物理ベースで描画するスカイレンダラーです。掲載シーンはすべて決定論的・再現可能な
セッションです。リポジトリをクローンしてコマンドを実行すれば、同じピクセルが
得られます。

全プリセット（ビジュアル回帰用のテクニカルなベースラインを含む）は
[`validation-gallery.md`](validation-gallery.md) にあります。このページは
プロジェクト玄関口として、見栄えのするシーンを厳選したサブセットです。

## 任意のシーンを再現

```bash
cargo run -p stars-cli --release -- --preset <name> -o out.png
```

キュレーション済みセットを一括再描画:

```bash
make demo-gallery
```

コミット済み PNG は 480 × 270（`docs/assets/validation` と同じ解像度）。
スクリプトを直接呼ぶときは `STARS_DEMO_GALLERY_WIDTH` /
`STARS_DEMO_GALLERY_HEIGHT` で上書きできます。

## キュレーションシーン

### 🌆 Tokyo tonight（東京の夏の夕）

![Tokyo summer evening](assets/demo-gallery/tokyo-tonight.png)

デフォルトのローカル視点: 北緯 35.68°・東経 139.69°・2026-06-21 夕方、
全オーバーレイ ON。ユーザーが最初に見る入口画面。カタログ色は B−V →
黒体スペクトル → sRGB（V-23）、Kasten-Young 消光（V-37）、惑星ラベル、
地平線、黄道を表示。

Preset: `tokyo-tonight`

---

### 🌅 Sunset horizon（日没）

![Sunset horizon](assets/demo-gallery/sunset.png)

ゴールデンアワーの散乱: Preetham の解析的 Sun-lit Sky（V-32）と地平線の
ヘイズが滑らかに繋がる。低太陽高度の市民薄明バンドまで連続性を保つ。

Preset: `sunset`

---

### 🌄 Belt of Venus（反太陽方向、東京）

![Civil twilight anti-solar Tokyo](assets/demo-gallery/civil-twilight-antisolar-tokyo.png)

市民薄明時に反太陽方向を望んだピンクの Belt of Venus アーチと、その下の
青灰色の地球影バンド（V-27）。Lee & Hernández-Andrés 2003 の anti-twilight
測光フィットに準拠。

Preset: `civil-twilight-antisolar-tokyo`

---

### 🌙 Moonlit night（月明かりの夜）

![Moonlit night](assets/demo-gallery/moonlit-night.png)

ランバート反射の月面に、未照射半球側にチャネル別 earthshine（"Da Vinci
glow"、V-26）が加算され、さらに月明加算スカイ項が乗る。月光照度は
Krisciunas-Schaefer 1991。

Preset: `moonlit-night`

---

### 🌌 Dark sky（暗夜空・天の川）

![High-altitude dark sky](assets/demo-gallery/dark-sky.png)

高標高の田園夜空: Leinert 1998 の積分星明かり、黄道光、3 成分スペクトル
大気光（V-28: O I 557.7 nm 緑、Na D 589 nm 黄、OH Meinel 赤/近赤）、
そして SFD ダスト減光が天の川バンドを通る。

Preset: `dark-sky`

---

### 🏙️ 光害比較（Bortle 1 vs Bortle 8）

| Bortle 1（田園） | Bortle 8（都市夜空） |
|---|---|
| ![Bortle 1 rural floor](assets/demo-gallery/dark-sky-bortle-1.png) | ![Tokyo Bortle 8](assets/demo-gallery/tokyo-bortle-8.png) |

V-39 Bortle / SQM 観測地側スケーリング: 同じレンダラー・同じ日時・同じ
座標。Bortle 8 では Garstang の水平線方向増光カーネルの上に
ナトリウム/LED のウォーム橙色ティントが重なり、暗い星は自然光と同じ
Kasten-Young 消光で減光される。

Presets: `dark-sky-bortle-1`, `tokyo-bortle-8`

---

### ☀️ Total solar eclipse（皆既日食）

![Total solar eclipse](assets/demo-gallery/solar-eclipse.png)

V-51c 統一日食/掩蔽パイプライン: 解析マスクによる Moon-on-Sun 減算、
Koomen 1952 の昼空減光（被覆された太陽フラックスに比例して天空輝度を
スケール）、皆既中の Baumbach 1937 コロナ。

Preset: `solar-eclipse`

---

### 🪐 Venus transit of the Sun（金星の太陽面通過）

![Venus transit](assets/demo-gallery/venus-transit.png)

V-51e の planet-on-Sun トランジット: 金星の見かけ視円が解析的 occluder
アレイ（V-51b）に小さな黒点として入る。視円外の昼空帯は変化しない。

Preset: `venus-transit`

---

### 🌖 Galilean shadow on Jupiter（木星面のガリレオ衛星影）

![Jupiter Galilean shadow transit](assets/demo-gallery/jupiter-shadow-transit.png)

V-52d: 太陽投影されたガリレオ衛星位置が、木星をターゲットにした小さな
occluder を発する。衛星スプライト自体は木星の裏側に来た時に cull
される。2008-12-20 の Io 影侵入を JPL Horizons に対してピン留め。

Preset: `jupiter-shadow-transit`

---

### 🗺️ Full-sky Mollweide（全天モルワイデ図法）

![Mollweide all-sky map](assets/demo-gallery/all-sky-mollweide.png)

天球全体の等積モルワイデ投影（V-40）: 天の川バンド、ダスト減光、座標
グリッドが ±180° の方位ラップを跨いで連続。

Preset: `all-sky-mollweide`

---

### 🌐 Galactic-north viewpoint（銀河北極視点）

![Galactic-north viewpoint](assets/demo-gallery/galactic-north.png)

銀河北極を見下ろす外部視点（V-41）: HYG カタログの距離が解析的
天の川円盤の周りに距離スケーリング済み星野を駆動する。地球側シーンと
同じカメラ Uniform パイプラインを使い、視点原点だけが変わる。

Preset: `galactic-north`

## 由来 (Provenance)

掲載 PNG はすべて [`data/manifest.toml`](../data/manifest.toml) に
`kind = "generated"` および `preprocessing = "scripts/render-demo-gallery.sh"`
で記録されています。`make manifest-check`（`make ci` に含まれる）が
コミット済みバイト列を再ハッシュし、サイレントドリフトを失敗させます。

シーンプリセットのセッション JSON は `docs/presets/sessions/` 配下にあります。
カタログ・暦・大気モデルのバージョンは各セッションファイルに含まれて
いるため、将来再描画しても同じ科学的状態を再現できます。
