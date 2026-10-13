# 五笔学堂（wubi-mentor）

学习 86 版五笔拆字的 macOS 桌面程序，基于 [GPUI Fast](https://github.com/longbridge/gpui-fast)
和 [GPUI Kit](https://github.com/longbridge/gpui-kit)。

灵感来自 [wongsingfo/86wubi](https://github.com/wongsingfo/86wubi)：用自己的文本练习，
边打边看拆字。不同的是这里的拆分是结构化数据，不是图片：

- **练习**：稿纸式文字格，用系统的五笔输入法直接打字，程序逐字比对；输入法正在组字的编码显示在当前字下面。
  右侧拆字卡显示字根序列、每一码的来源（字根 / 识别码 / 首、次、末笔）、简码；底部键盘高亮编码顺序。
  提示模式可选：始终提示 / 出错再提示（停顿 6 秒也会提示）/ 不提示（Tab 偷看）。
  打错的字会标出来，点任意字可以查看它的拆分或从这里开始练。
- **文本库**：新建、粘贴、编辑练习文本；一键用最近的薄弱字生成“错字重练”。
- **查字**：输入任意文字逐字显示拆分；空白时显示 86 版字根键盘和取码规则速记。
- **统计**：累计字数和用时、速度/一次打对率曲线、每次练习记录、薄弱字。

## 运行

```bash
cargo run --release
```

需要 Rust 1.98.1（`rust-toolchain.toml` 会自动选择，gpui-fast 用到了较新的标准库特性）。

## 数据

所有数据都在 SQLite 里：`~/Library/Application Support/wubi-mentor/wubi.db`（可用环境变量 `WUBI_DB` 指定）。

| 表 | 内容 |
| --- | --- |
| `chars` | 86 版字典：全码、简码、类别、字根拆分、识别码（首次启动时从 `data/wubi86.tsv` 导入） |
| `texts` | 练习文本和每篇的当前进度 |
| `char_stats` | 每个字的练习次数、出错次数、用时、连对次数（据此判断掌握程度） |
| `sessions` | 每次练习的字数、有效用时、一次打对数 |
| `meta` | 设置（当前文本、提示模式）和字典版本 |

`data/wubi86.tsv` 由 `scripts/build_dict.py` 生成：

- 拆分来自 [gmgi1234/wubi](https://github.com/gmgi1234/wubi) 的 `wubi86.js`，脚本把其中的部件递归展开到基本字根；
- 编码用 [iHTCboy/iWuBi](https://github.com/iHTCboy/iWuBi) 的 86 版码表交叉校验，并补上简码。

9500 多个字里约 99.3% 的拆分和编码能互相印证；对不上的字在拆字卡上会标注“仅供参考”。
常用字 3500 字全部收录。

字根用「黑体字根」字体显示（`assets/fonts/wubi-roots.ttf`），它把 86 版字根和各种变形部件放在
Unicode 私用区。字体和逐码字形数据（`wb_spelling.dict.yaml`）来自
[myshiqiqi/rime-wubi](https://github.com/myshiqiqi/rime-wubi)；原字体没有有效的 PostScript 名、
也没有字母 m（GPUI 会因此跳过它），`scripts/patch_root_font.py` 修正了这两点。
参与编码的字根直接用逐码字形，中间不取码的字根按全库统计映射到最常对应的字形。
上述来源都没有声明许可证，自用没问题，公开发布前请留意。

重新生成数据：

```bash
python3 scripts/build_dict.py <下载目录> data/wubi86.tsv "$(cat scripts/traditional_roots.txt)"
python3 scripts/patch_root_font.py <黑体字根.ttf> assets/fonts/wubi-roots.ttf
```

## 开发

```bash
cargo test
# 按脚本操作界面并把每一步渲染成 PNG（不需要屏幕录制权限）
WUBI_DB=/tmp/snap.db WUBI_SNAPSHOT=/tmp/snap \
WUBI_SNAPSHOT_SCRIPT="open 常用字; type 学习五笔; mark rqyy; lookup 照键; page stats" \
cargo run --features snapshot
```
