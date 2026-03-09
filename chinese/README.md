# RFM — Rust 文件管理器

基于终端的双面板文件管理器，灵感来源于 Total Commander，使用 Rust 编写。

## 简介

RFM 可在任何 Linux 发行版的终端中运行。

**特性：**
- 两个独立面板：左面板和右面板
- 配色方案：目录 — 蓝色，文件 — 绿色，已选 — 黄色
- 通过 `~/.config/rfm/keymap.toml` 完全自定义快捷键
- 通过 `~/.config/rfm/openers.toml` 按扩展名配置文件打开方式
- USB 驱动器菜单，自动检测挂载点
- 文件搜索，支持前进/后退导航
- 权限编辑器（chmod），支持 sudo 和递归模式
- 复制和移动操作时显示进度条

## 编译

```bash
cargo build --release
```

## 安装

```bash
sudo cp target/release/rfm /usr/local/bin/rfm
```

## 启动

```bash
rfm
```

## 配置

配置文件位于 `~/.config/rfm/`

| 文件 | 说明 |
|------|------|
| `keymap.toml` | 快捷键配置 |
| `openers.toml` | 按扩展名配置文件打开程序（可在应用内通过 `Alt+Enter` 设置） |

`openers.toml` 示例：
```toml
[openers]
pdf  = "evince"
mp4  = "mpv"
txt  = "kate"
png  = "eog"
zip  = "ark"
```

## 快捷键

### 导航

| 按键 | 功能 |
|------|------|
| `↑` / `↓` | 移动光标 |
| `Alt+↑` | 向上跳转（默认：5 行） |
| `Alt+↓` | 向下跳转（默认：5 行） |
| `PageUp` / `PageDown` | 向上/向下翻页 |
| `g` | 跳到列表顶部 |
| `G` | 跳到列表底部 |
| `Enter` / `→` | 进入目录 / 打开文件 |
| `Backspace` / `←` | 返回上级目录 |
| `\` | 跳转到根目录 `/` |

### 面板

| 按键 | 功能 |
|------|------|
| `Tab` | 切换面板 |
| `[` | 聚焦左面板 |
| `]` | 聚焦右面板 |

### 文件操作

| 按键 | 功能 |
|------|------|
| `空格` | 选择 / 取消选择文件 |
| `F5` | 复制已选文件到另一面板 |
| `F6` | 移动已选文件到另一面板 |
| `F7` | 新建目录 |
| `F8` | 删除已选文件（需确认） |
| `F2` | 新建文件 |
| `r` | 刷新两个面板 |

### 搜索

| 按键 | 功能 |
|------|------|
| `F3` | 打开搜索面板 |
| `Enter` | 查找下一个匹配项 |
| `Esc` | 关闭搜索 |

### 权限

| 按键 | 功能 |
|------|------|
| `Alt+f` | 打开权限对话框（chmod） |
| `空格` | 切换复选框 |
| `Enter` | 应用（必要时使用 sudo） |

### USB 驱动器

| 按键 | 功能 |
|------|------|
| `Alt+u` | 打开 USB 驱动器菜单 |
| `Enter` | 导航到挂载点 |
| `Esc` | 关闭菜单 |

### 其他

| 按键 | 功能 |
|------|------|
| `Alt+Enter` | 设置或更改文件扩展名的打开程序 |
| `q` | 退出程序 |

## 快捷键配置

`~/.config/rfm/keymap.toml` 示例：

```toml
[keys]
quit          = "q"
panel_switch  = "Tab"
panel_left    = "["
panel_right   = "]"
go_up         = "Backspace"
enter         = "Enter"
jump_up       = "Alt+Up"
jump_down     = "Alt+Down"
jump_amount   = 5
go_top        = "g"
go_bottom     = "G"
page_up       = "PageUp"
page_down     = "PageDown"
select        = "Space"
copy          = "F5"
move_files    = "F6"
mkdir         = "F7"
delete        = "F8"
create_file   = "F2"
search        = "F3"
chmod         = "Alt+f"
usb_menu      = "Alt+u"
refresh       = "r"
go_root       = "\\"
```

支持的按键名称：`Enter`、`Tab`、`Backspace`、`Esc`、`Space`、`Delete`、
`Up`、`Down`、`Left`、`Right`、`Home`、`End`、`PageUp`、`PageDown`、
`F1`–`F10`、`Alt+X`、`Ctrl+X`，或任意单个字符。
